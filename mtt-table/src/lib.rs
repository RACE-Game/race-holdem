mod errors;

use std::collections::btree_map::Entry;

use borsh::{BorshDeserialize, BorshSerialize};
use race_api::event::BridgeEvent;
use race_api::prelude::*;
use race_holdem_base::essential::{GameMode, Player, PlayerStatus};
use race_holdem_base::game::Holdem;
use race_holdem_mtt_base::{
    ChipsChange, HoldemBridgeEvent, MttTablePlayer, MttTableSitin, MttTableState, PlayerResult, PlayerResultStatus
};
use race_proc_macro::game_handler;

pub type PlayerId = u64;

#[game_handler]
#[derive(Debug, BorshSerialize, BorshDeserialize, Default, Clone)]
pub struct MttTable {
    pub table_id: GameId,
    pub holdem: Holdem,
}

impl GameHandler for MttTable {
    fn balances(&self) -> Vec<PlayerBalance> {
        self.holdem.balances()
    }

    fn init_state(init_account: InitAccount) -> HandleResult<Self> {
        let MttTableState {
            sb,
            bb,
            players,
            table_id,
            btn,
            ..
        } = init_account.data()?;

        let player_map = players
            .into_iter()
            .map(|p| {
                (
                    p.id,
                    Player::new_with_timeout(p.id, p.chips, p.table_position as _, 0),
                )
            })
            .collect();

        let holdem = Holdem {
            btn,
            sb,
            bb,
            table_size: init_account.max_players as _,
            mode: GameMode::Mtt,
            player_map,
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
                            PlayerStatus::Out => PlayerResultStatus::Sitout,
                            PlayerStatus::Eliminated => PlayerResultStatus::Eliminated,
                            _ => PlayerResultStatus::Normal,
                        };
                        let chips_change = match self.holdem.hand_history.chips_change.get(&p.id) {
                            Some(race_holdem_base::hand_history::ChipsChange::NoUpdate) => None,
                            Some(race_holdem_base::hand_history::ChipsChange::Add(amt)) => Some(ChipsChange::Add(*amt)),
                            Some(race_holdem_base::hand_history::ChipsChange::Sub(amt)) => Some(ChipsChange::Sub(*amt)),
                            None => None,
                        };
                        player_results.push(PlayerResult::new(p.id, p.chips, chips_change, status));
                    }

                    self.holdem.kick_players(effect);

                    let mtt_table_state = self.make_mtt_table_state();
                    let evt = HoldemBridgeEvent::GameResult {
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
            .map(|p| {
                MttTablePlayer::new(
                    p.id,
                    p.chips,
                    p.position as _,
                )
            })
            .collect();

        MttTableState {
            table_id: self.table_id,
            btn: self.holdem.btn,
            hand_id: self.holdem.hand_id,
            sb: self.holdem.sb,
            bb: self.holdem.bb,
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
                sitout_players,
            } => {
                let timeout = self
                    .holdem
                    .next_game_start
                    .saturating_sub(effect.timestamp());
                self.holdem.sb = sb;
                self.holdem.bb = bb;
                for id in sitout_players.iter() {
                    match self.holdem.player_map.entry(*id) {
                        Entry::Vacant(_) => return Err(errors::invalid_player_in_start_game()),
                        Entry::Occupied(e) => {
                            e.remove();
                        }
                    }
                }
                effect.wait_timeout(timeout);
            }
            // Add players from other tables
            HoldemBridgeEvent::SitinPlayers { sitins } => {

                let origin_num_of_players = self.holdem.player_map.len();

                for sitin in sitins.into_iter() {
                    let MttTableSitin { id, chips } = sitin;

                    if let Some(position) = self.holdem.find_position() {
                        match self.holdem.player_map.entry(id) {
                            Entry::Vacant(e) => e.insert(Player::new_with_timeout_and_status(
                                id,
                                chips,
                                position as usize,
                                PlayerStatus::Init,
                            )),
                            Entry::Occupied(_) => {
                                return Err(errors::duplicated_player_in_relocate())
                            }
                        };
                    }
                }

                if origin_num_of_players < 2 {
                    // The game is supposed to be halted. We should add the new player then start the game.
                    effect.checkpoint();
                    effect.bridge_event(0, HoldemBridgeEvent::GameResult {
                        hand_id: self.holdem.hand_id,
                        table_id: self.table_id,
                        player_results: vec![],
                        table: self.make_mtt_table_state(),
                    })?;

                    if self.holdem.player_map.len() >= 2 {
                    let timeout = self
                        .holdem
                        .next_game_start
                        .saturating_sub(effect.timestamp());
                    effect.wait_timeout(timeout);
                    }
                } else {
                    // The game will receive a StartGame anyway. The only thing we do is to add the new players.
                }
            }
            HoldemBridgeEvent::CloseTable => {
                self.holdem.player_map.clear();
                effect.checkpoint();
            }
            _ => return Err(errors::internal_invalid_bridge_event()),
        };
        Ok(())
    }
}

#[cfg(test)]
mod tests {

    use super::*;
    use borsh::BorshDeserialize;
    use race_holdem_mtt_base::{HoldemBridgeEvent, MttTablePlayer, MttTableState};

    fn default_players(num_of_players: usize) -> Vec<MttTablePlayer> {
        (1..=num_of_players).map(|n| MttTablePlayer::new(n as u64, (n + 1) as u64 * 500, n - 1)).collect()
    }

    fn default_mtt_table_state(num_of_players: usize) -> MttTableState {
        MttTableState {
            sb: 100,
            bb: 200,
            players: default_players(num_of_players),
            table_id: 1,
            btn: 0,
            ..Default::default()
        }
    }

    fn mtt_table_with_players(num_of_players: usize) -> MttTable {
        let init_data = default_mtt_table_state(num_of_players);

        let init_account = InitAccount {
            max_players: 9,
            data: borsh::to_vec(&init_data).unwrap(),
        };

        let mtt_table = MttTable::init_state(init_account).unwrap();
        mtt_table
    }

    #[test]
    fn test_init_state() {
        let init_data = MttTableState {
            sb: 100,
            bb: 200,
            players: vec![
                MttTablePlayer::new(1, 1000, 0),
                MttTablePlayer::new(2, 1500, 1),
            ],
            table_id: 1,
            btn: 0,
            ..Default::default()
        };

        let init_account = InitAccount {
            max_players: 9,
            data: borsh::to_vec(&init_data).unwrap(),
        };

        let mtt_table = MttTable::init_state(init_account).unwrap();
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
            sitout_players: vec![999], // Invalid player ID
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
            sitout_players: vec![1, 2],
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
                MttTableSitin::new(1, 1000), // Duplicate player
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
                MttTableSitin::new(3, 3000), // Player 3 is already on the table
            ],
        };

        let result = mtt_table.handle_bridge_event(&mut effect, bridge_event);

        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err(),
            errors::duplicated_player_in_relocate()
        );
    }

    #[test]
    fn test_handle_bridge_event_relocate_in_init_stage() {
        let mut mtt_table = mtt_table_with_players(3);
        let mut effect = Effect::default();

        let sitins = vec![
            MttTableSitin::new(4, 1000),
            MttTableSitin::new(5, 1500),
        ];

        let bridge_event = HoldemBridgeEvent::SitinPlayers { sitins };

        mtt_table
            .handle_bridge_event(&mut effect, bridge_event)
            .unwrap();

        assert_eq!(mtt_table.holdem.player_map.len(), 5);
        assert!(mtt_table.holdem.player_map.contains_key(&4));
        assert!(mtt_table.holdem.player_map.contains_key(&5));
        assert_eq!(effect.wait_timeout, None);
        assert_eq!(effect.list_bridge_events::<HoldemBridgeEvent>().unwrap(), vec![]);
    }

    #[test]
    fn test_handle_bridge_event_relocate_in_init_stage_with_0_player() {
        let mut mtt_table = mtt_table_with_players(0);
        let mut effect = Effect::default();

        let sitins = vec![
            MttTableSitin::new(1, 500),
        ];

        let bridge_event = HoldemBridgeEvent::SitinPlayers { sitins };

        mtt_table
            .handle_bridge_event(&mut effect, bridge_event)
            .unwrap();

        assert_eq!(mtt_table.holdem.player_map.len(), 1);
        assert!(mtt_table.holdem.player_map.contains_key(&1));
        assert!(effect.is_checkpoint());
        assert_eq!(effect.wait_timeout, None);
        assert_eq!(effect.list_bridge_events().unwrap(), vec![
            (0, HoldemBridgeEvent::GameResult {
                hand_id:mtt_table.holdem.hand_id,
                table_id:mtt_table.table_id,
                player_results: vec![],
                table: mtt_table.make_mtt_table_state(),
            })
        ]);
    }

    #[test]
    fn test_handle_bridge_event_relocate_in_init_stage_with_1_player() {
        let mut mtt_table = mtt_table_with_players(1);
        let mut effect = Effect::default();

        let sitins = vec![
            MttTableSitin::new(2, 500),
        ];

        let bridge_event = HoldemBridgeEvent::SitinPlayers { sitins };

        mtt_table
            .handle_bridge_event(&mut effect, bridge_event)
            .unwrap();

        assert_eq!(mtt_table.holdem.player_map.len(), 2);
        assert!(mtt_table.holdem.player_map.contains_key(&1));
        assert!(mtt_table.holdem.player_map.contains_key(&2));
        assert!(effect.is_checkpoint());
        assert_eq!(effect.wait_timeout, Some(0));
        assert_eq!(effect.list_bridge_events().unwrap(), vec![
            (0, HoldemBridgeEvent::GameResult {
                hand_id:mtt_table.holdem.hand_id,
                table_id:mtt_table.table_id,
                player_results: vec![],
                table: mtt_table.make_mtt_table_state(),
            })
        ]);
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

    #[test]
    fn test_handle_event_with_checkpoint() {
        let mut mtt_table = mtt_table_with_players(3);
        let mut effect = Effect::default();
        let event = Event::Ready; // The event doesn't really matter here.
        effect.checkpoint(); // Set checkpoint

        mtt_table.handle_event(&mut effect, event).unwrap();

        let expected_event = HoldemBridgeEvent::GameResult {
            hand_id: 0,
            table: MttTableState {
                table_id: 1,
                btn: 0,
                hand_id: 0,
                sb: 100,
                bb: 200,
                next_game_start: 0,
                players: default_players(3),
            },
            player_results: vec![
                PlayerResult::new(1, 1000, None, PlayerResultStatus::Normal),
                PlayerResult::new(2, 1500, None, PlayerResultStatus::Normal),
                PlayerResult::new(3, 2000, None, PlayerResultStatus::Normal),
            ],
            table_id: 1,
        };

        let bridge_event = effect.bridge_events.pop().unwrap();
        let actual_event: HoldemBridgeEvent =
            BorshDeserialize::try_from_slice(&bridge_event.raw).unwrap();

        assert_eq!(expected_event, actual_event);
    }
}
