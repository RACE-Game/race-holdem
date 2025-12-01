mod errors;

use std::collections::btree_map::Entry;

use borsh::{BorshDeserialize, BorshSerialize};
use race_api::event::BridgeEvent;
use race_api::prelude::*;
use race_poker_base::essential::{GameMode, Player, PlayerStatus};
use race_poker_base::game::PokerGame;
use race_poker_base::holdem::HoldemVariant;
use race_holdem_mtt_base::{
    ChipsChange, HoldemBridgeEvent, MttTableInit, MttTablePlayer, MttTableSitin, MttTableState, PlayerResult,
    PlayerResultStatus,
};
use race_proc_macro::game_handler;

pub type PlayerId = u64;

#[game_handler]
#[derive(Debug, BorshSerialize, BorshDeserialize, Default, Clone)]
pub struct MttTable {
    pub table_id: usize,
    pub holdem: PokerGame<HoldemVariant>,
}

impl GameHandler for MttTable {
    fn balances(&self) -> Vec<PlayerBalance> {
        self.holdem.balances()
    }

    fn init_state(_effect: &mut Effect, init_account: InitAccount) -> HandleResult<Self> {
        let MttTableInit {
            table_id, sb, bb, ante, players, max_afk_hands,
        } = init_account.data()?;

        let player_map = players
            .into_iter()
            .enumerate()
            .map(|(idx, p)| {
                let mut player = Player::new_with_timeout(p.id, p.chips, idx as u8, 0);
                player.is_afk = true;
                (
                    p.id,
                    player,
                )
            })
            .collect();

        let holdem = PokerGame::<HoldemVariant> {
            btn: 0,
            sb,
            bb,
            ante,
            table_size: init_account.max_players as _,
            mode: GameMode::Mtt,
            player_map,
            max_afk_hands,
            ..Default::default()
        };

        Ok(Self { table_id, holdem })
    }

    fn handle_event(&mut self, effect: &mut Effect, event: Event) -> HandleResult<()> {
        match event {
            Event::Bridge {
                dest_game_id, raw, ..
            } => {
                self.holdem.display.clear();
                if dest_game_id == self.table_id as _ {
                    let bridge_event = HoldemBridgeEvent::try_parse(&raw)?;
                    self.handle_bridge_event(effect, bridge_event)?;
                } else {
                    Err(errors::invalid_bridge_event())?
                }
            }
            _ => {
                self.holdem.handle_event(effect, event)?;
                // Check if there's a checkpoint
                if effect.is_checkpoint() {
                    let mut player_results = Vec::new();

                    for p in self.holdem.player_map.values() {
                        let status = match p.status {
                            PlayerStatus::Leave => PlayerResultStatus::Sitout,
                            PlayerStatus::Eliminated => PlayerResultStatus::Eliminated,
                            _ => PlayerResultStatus::Normal,
                        };
                        let chips_change = match self.holdem.hand_history.chips_change.get(&p.id) {
                            Some(race_poker_base::hand_history::ChipsChange::NoUpdate) => None,
                            Some(race_poker_base::hand_history::ChipsChange::Add(amt)) => {
                                Some(ChipsChange::Add(*amt))
                            }
                            Some(race_poker_base::hand_history::ChipsChange::Sub(amt)) => {
                                Some(ChipsChange::Sub(*amt))
                            }
                            None => None,
                        };
                        player_results.push(PlayerResult::new(p.id, p.chips, chips_change, p.position, status, p.timeout));
                    }

                    self.holdem.kick_players(effect);
                    self.holdem.reset_state()?;

                    let mtt_table_state = self.make_mtt_table_state();
                    let evt = HoldemBridgeEvent::GameResult {
                        btn: self.holdem.btn,
                        hand_id: self.holdem.hand_id,
                        table: mtt_table_state,
                        player_results,
                        table_id: self.table_id,
                    };
                    effect.bridge_event(0, evt)?;
                }
            }
        };

        Ok(())
    }
}

impl MttTable {
    fn make_mtt_table_state(&self) -> MttTableState {
        let players = self
            .holdem
            .player_map
            .values()
            .map(|p| MttTablePlayer::new(p.id, p.chips, p.position, p.time_cards))
            .collect();

        MttTableState {
            table_id: self.table_id,
            btn: self.holdem.btn,
            hand_id: self.holdem.hand_id,
            sb: self.holdem.sb,
            bb: self.holdem.bb,
            ante: self.holdem.ante,
            next_game_start: self.holdem.next_game_start,
            players,
        }
    }

    fn handle_bridge_event(
        &mut self,
        effect: &mut Effect,
        event: HoldemBridgeEvent,
    ) -> HandleResult<()> {
        match event {
            HoldemBridgeEvent::StartGame {
                sb,
                bb,
                ante,
                sitout_players,
                start_time,
            } => {
                self.holdem.sb = sb;
                self.holdem.bb = bb;
                self.holdem.ante = ante;
                for id in sitout_players.iter() {
                    match self.holdem.player_map.entry(*id) {
                        Entry::Vacant(_) => return Err(errors::invalid_player_in_start_game()),
                        Entry::Occupied(e) => {
                            e.remove();
                        }
                    }
                }
                if self.holdem.player_map.len() > 1 {
                    if let Some(start_time) = start_time {
                        effect.wait_timeout(start_time - effect.timestamp());
                    } else {
                        effect.start_game();
                    }
                }
            }
            // Add players from other tables
            HoldemBridgeEvent::SitinPlayers { sitins } => {
                let origin_num_of_players = self.holdem.player_map.len();

                for sitin in sitins.into_iter() {

                    let MttTableSitin {
                        id,
                        chips,
                        time_cards,
                        first_time_sit,
                    } = sitin;

                    if let Some(position) = self.holdem.find_position() {

                        // The player's init status depends on where the player is seated
                        // If the player sits between BTN/SB and SB/BB, the status is default to WAITBB
                        // Otherwise, it is INIT.
                        let status;
                        if first_time_sit {
                            status = PlayerStatus::Waitbb;
                        } else {
                            status = PlayerStatus::Init;
                        }
                        match self.holdem.player_map.entry(id) {
                            Entry::Vacant(e) => e.insert(Player::new(
                                id,
                                chips,
                                position,
                                status,
                                0,
                                0,
                                time_cards,
                            )),
                            Entry::Occupied(_) => {
                                return Err(errors::duplicated_player_in_relocate())
                            }
                        };
                    } else {
                        effect.error(format!("sit player error {:?}", sitin));
                    }
                }

                if origin_num_of_players < 2 {
                    // The game is supposed to be halted.
                    // We send a GameResult to trigger the StartGame instead of start directly here.
                    effect.checkpoint();
                    effect.bridge_event(
                        0,
                        HoldemBridgeEvent::GameResult {
                            hand_id: self.holdem.hand_id,
                            table_id: self.table_id,
                            btn: self.holdem.btn,
                            player_results: vec![],
                            table: self.make_mtt_table_state(),
                        },
                    )?;
                }
            }
            HoldemBridgeEvent::CloseTable => {
                self.holdem.player_map.clear();
                effect.checkpoint();
                effect.stop_game();
            }
            _ => return Err(errors::internal_invalid_bridge_event()),
        };
        Ok(())
    }
}

#[cfg(test)]
mod tests {

    use super::*;
    use race_holdem_mtt_base::{HoldemBridgeEvent, MttTableInit, MttTablePlayerInit};

    fn default_players(num_of_players: usize) -> Vec<MttTablePlayerInit> {
        (1..=num_of_players)
            .map(|n| MttTablePlayerInit::new(n as u64, (n + 1) as u64 * 500))
            .collect()
    }

    fn default_mtt_table_state(num_of_players: usize) -> MttTableInit {
        MttTableInit {
            sb: 100,
            bb: 200,
            ante: 0,
            players: default_players(num_of_players),
            table_id: 1,
            max_afk_hands: 100,
        }
    }

    fn mtt_table_with_players(num_of_players: usize) -> MttTable {
        let init_data = default_mtt_table_state(num_of_players);
        let mut effect = Effect::default();

        let init_account = InitAccount {
            max_players: 9,
            data: borsh::to_vec(&init_data).unwrap(),
        };

        let mtt_table = MttTable::init_state(&mut effect, init_account).unwrap();
        mtt_table
    }

    #[test]
    fn test_init_state() {
        let init_data = MttTableInit {
            sb: 100,
            bb: 200,
            players: vec![
                MttTablePlayerInit::new(1, 1000),
                MttTablePlayerInit::new(2, 1500),
            ],
            table_id: 1,
            ..Default::default()
        };

        let init_account = InitAccount {
            max_players: 9,
            data: borsh::to_vec(&init_data).unwrap(),
        };

        let mut effect = Effect::default();

        let mtt_table = MttTable::init_state(&mut effect, init_account).unwrap();
        assert_eq!(mtt_table.table_id, 1);
        assert_eq!(mtt_table.holdem.sb, 100);
        assert_eq!(mtt_table.holdem.bb, 200);
        assert_eq!(mtt_table.holdem.table_size, 9);
        assert_eq!(mtt_table.holdem.player_map.len(), 2);
    }

    #[test]
    fn test_start_game_invalid_player_id() {
        let mut mtt_table = mtt_table_with_players(3);
        let mut effect = Effect::default();
        let invalid_player_id_event = HoldemBridgeEvent::StartGame {
            sb: 100,
            bb: 200,
            ante: 0,
            sitout_players: vec![999], // Invalid player ID
            start_time: None,
        };
        let result = mtt_table.handle_bridge_event(&mut effect, invalid_player_id_event);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), errors::invalid_player_in_start_game());
    }

    #[test]
    fn test_handle_bridge_event_start_game() {
        let mut mtt_table = mtt_table_with_players(3);
        let mut effect = Effect::default();

        let bridge_event = HoldemBridgeEvent::StartGame {
            sb: 100,
            bb: 200,
            ante: 0,
            sitout_players: vec![1, 2],
            start_time: None,
        };

        mtt_table
            .handle_bridge_event(&mut effect, bridge_event)
            .unwrap();

        assert_eq!(mtt_table.holdem.sb, 100);
        assert_eq!(mtt_table.holdem.bb, 200);
        assert_eq!(mtt_table.holdem.player_map.len(), 1);
    }

    #[test]
    fn test_handle_bridge_event_relocate_duplicate_player() {
        let mut mtt_table = mtt_table_with_players(3);
        let mut effect = Effect::default();

        // Attempt to relocate the same player to the table
        let bridge_event = HoldemBridgeEvent::SitinPlayers {
            sitins: vec![
                MttTableSitin::new_with_defaults(1, 1000), // Duplicate player
            ],
        };

        let result = mtt_table.handle_bridge_event(&mut effect, bridge_event);

        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), errors::duplicated_player_in_relocate());
    }

    #[test]
    fn test_handle_bridge_event_relocate_duplicate_position() {
        let mut mtt_table = mtt_table_with_players(3);
        let mut effect = Effect::default();

        // Attempt to relocate a player to an already occupied position
        let bridge_event = HoldemBridgeEvent::SitinPlayers {
            sitins: vec![
                MttTableSitin::new_with_defaults(3, 3000), // Player 3 is already on the table
            ],
        };

        let result = mtt_table.handle_bridge_event(&mut effect, bridge_event);

        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), errors::duplicated_player_in_relocate());
    }

    #[test]
    fn test_handle_bridge_event_relocate_in_init_stage() {
        let mut mtt_table = mtt_table_with_players(3);
        let mut effect = Effect::default();

        let sitins = vec![
            MttTableSitin::new_with_defaults(4, 1000),
            MttTableSitin::new_with_defaults(5, 1500),
        ];

        let bridge_event = HoldemBridgeEvent::SitinPlayers { sitins };

        mtt_table
            .handle_bridge_event(&mut effect, bridge_event)
            .unwrap();

        assert_eq!(mtt_table.holdem.player_map.len(), 5);
        assert!(mtt_table.holdem.player_map.contains_key(&4));
        assert!(mtt_table.holdem.player_map.contains_key(&5));
        assert_eq!(effect.wait_timeout, None);
        assert_eq!(
            effect.list_bridge_events::<HoldemBridgeEvent>().unwrap(),
            vec![]
        );
    }

    #[test]
    fn test_handle_bridge_event_relocate_in_init_stage_with_0_player() {
        let mut mtt_table = mtt_table_with_players(0);
        let mut effect = Effect::default();

        let sitins = vec![MttTableSitin::new_with_defaults(1, 500)];

        let bridge_event = HoldemBridgeEvent::SitinPlayers { sitins };

        mtt_table
            .handle_bridge_event(&mut effect, bridge_event)
            .unwrap();

        assert_eq!(mtt_table.holdem.player_map.len(), 1);
        assert!(mtt_table.holdem.player_map.contains_key(&1));
        assert!(effect.is_checkpoint());
        assert_eq!(effect.wait_timeout, None);
        assert_eq!(
            effect.list_bridge_events().unwrap(),
            vec![(
                0,
                HoldemBridgeEvent::GameResult {
                    hand_id: mtt_table.holdem.hand_id,
                    table_id: mtt_table.table_id,
                    btn: mtt_table.holdem.btn,
                    player_results: vec![],
                    table: mtt_table.make_mtt_table_state(),
                }
            )]
        );
    }

    #[test]
    fn test_handle_bridge_event_relocate_in_init_stage_with_1_player() {
        let mut mtt_table = mtt_table_with_players(1);
        let mut effect = Effect::default();

        let sitins = vec![MttTableSitin::new_with_defaults(2, 500)];

        let bridge_event = HoldemBridgeEvent::SitinPlayers { sitins };

        mtt_table
            .handle_bridge_event(&mut effect, bridge_event)
            .unwrap();

        assert_eq!(mtt_table.holdem.player_map.len(), 2);
        assert!(mtt_table.holdem.player_map.contains_key(&1));
        assert!(mtt_table.holdem.player_map.contains_key(&2));
        assert!(effect.is_checkpoint());
        assert_eq!(
            effect.list_bridge_events().unwrap(),
            vec![(
                0,
                HoldemBridgeEvent::GameResult {
                    hand_id: mtt_table.holdem.hand_id,
                    table_id: mtt_table.table_id,
                    btn: mtt_table.holdem.btn,
                    player_results: vec![],
                    table: mtt_table.make_mtt_table_state(),
                }
            )]
        );
    }

    #[test]
    fn test_handle_bridge_event_close_table() {
        let mut mtt_table = MttTable::default();
        let mut effect = Effect::default();

        let bridge_event = HoldemBridgeEvent::CloseTable;

        mtt_table
            .handle_bridge_event(&mut effect, bridge_event)
            .unwrap();

        assert!(mtt_table.holdem.player_map.is_empty());
        assert!(effect.is_checkpoint());
    }
}
