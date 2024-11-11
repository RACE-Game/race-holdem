mod errors;

use std::collections::BTreeMap;

use borsh::{BorshDeserialize, BorshSerialize};
use race_api::event::BridgeEvent;
use race_api::prelude::*;
use race_holdem_base::essential::{GameMode, HoldemStage, Player, PlayerStatus};
use race_holdem_base::game::Holdem;
use race_holdem_mtt_base::{HoldemBridgeEvent, InitTableData, MttTableState, MttTablePlayer};
use race_proc_macro::game_handler;

pub type PlayerId = u64;

#[game_handler]
#[derive(Debug, BorshSerialize, BorshDeserialize, Default, Clone)]
pub struct MttTable {
    pub table_id: u8,
    pub hand_id: usize,
    pub holdem: Holdem,
}

impl GameHandler for MttTable {
    fn init_state(_effect: &mut Effect, init_account: InitAccount) -> HandleResult<Self> {
        if let Some(checkpoint) = init_account.checkpoint::<Self>()? {
            return Ok(checkpoint);
        }

        let InitTableData {
            table_id,
            start_sb,
            start_bb,
        } = init_account.data()?;

        let player_map: BTreeMap<u64, Player> = init_account
            .players
            .iter()
            .map(|p| (p.id, Player::new(p.id, p.balance, p.position, 0)))
            .collect();

        let holdem = Holdem {
            sb: start_sb,
            bb: start_bb,
            table_size: init_account.max_players as _,
            player_map,
            mode: GameMode::Mtt,
            ..Default::default()
        };

        Ok(Self {
            table_id,
            hand_id: 0,
            holdem,
        })
    }

    fn handle_event(&mut self, effect: &mut Effect, event: Event) -> HandleResult<()> {
        match event {
            Event::Bridge {
                dest,
                raw,
                join_players,
            } => {
                if dest == self.table_id as _ {
                    let bridge_event = HoldemBridgeEvent::try_parse(&raw)?;
                    self.handle_bridge_event(effect, bridge_event, join_players)?;
                }
            }
            _ => {
                self.holdem.handle_event(effect, event)?;
                // Check if there's a settlement
                if effect.is_checkpoint() {
                    let settles = effect.settles.clone();
                    let players = self
                        .holdem
                        .player_map
                        .values()
                        .map(|p| MttTablePlayer::new(p.id, p.chips, p.position as _))
                        .collect();
                    let mtt_table_state = MttTableState {
                        btn: self.holdem.btn,
                        hand_id: self.hand_id,
                        sb: self.holdem.sb,
                        bb: self.holdem.bb,
                        next_game_start: self.holdem.next_game_start,
                        players
                    };
                    let evt = HoldemBridgeEvent::GameResult {
                        hand_id: self.hand_id,
                        table: mtt_table_state,
                        settles,
                        table_id: self.table_id,
                    };
                    effect.checkpoint();
                    effect.bridge_event(0, evt, vec![])?;
                }
            }
        };

        Ok(())
    }
}

impl MttTable {
    fn handle_bridge_event(
        &mut self,
        effect: &mut Effect,
        event: HoldemBridgeEvent,
        _join_players: Vec<GamePlayer>,
    ) -> HandleResult<()> {
        match event {
            HoldemBridgeEvent::StartGame {
                sb,
                bb,
                moved_players,
            } => {
                let timeout = self
                    .holdem
                    .next_game_start
                    .checked_sub(effect.timestamp())
                    .unwrap_or(0);
                self.holdem.reset_state()?;
                self.holdem.sb = sb;
                self.holdem.bb = bb;
                for id in moved_players {
                    self.holdem.player_map.remove(&id);
                }
                effect.wait_timeout(timeout);
            }
            // Add players from other tables
            HoldemBridgeEvent::Relocate { players } => {
                for mtt_player in players.into_iter() {
                    let MttTablePlayer {
                        id,
                        chips,
                        table_position,
                    } = mtt_player;
                    self.holdem.player_map.insert(
                        id,
                        Player::new_with_status(id, chips, table_position as _, PlayerStatus::Init),
                    );
                }
                if self.holdem.stage == HoldemStage::Init {
                    let timeout = self
                        .holdem
                        .next_game_start
                        .checked_sub(effect.timestamp())
                        .unwrap_or(0);
                    effect.wait_timeout(timeout);
                }
            }
            HoldemBridgeEvent::CloseTable => {
                for (id, _) in self.holdem.player_map.iter() {
                    effect.settle(Settle::eject(*id))?;
                }
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

    #[test]
    fn test_init_game() -> anyhow::Result<()> {
        let st = vec![1, 50, 0, 0, 0, 0, 0, 0, 0, 100, 0, 0, 0, 0, 0, 0, 0];

        let st = InitTableData::try_from_slice(&st)?;

        println!("{:?}", st);

        Ok(())
    }

    // use std::collections::HashMap;

    // use super::*;
    // use race_holdem_base::essential::{GameEvent, Street};
    // use race_test::prelude::*;

    // #[test]
    // fn test_init_state() -> anyhow::Result<()> {
    //     let mut effect = Effect::default();
    //     let init_account = InitAccount {
    //         data: InitTableData {
    //             start_sb: 10,
    //             start_bb: 20,
    //             table_id: 1,
    //         }
    //             .try_to_vec()?,
    //         ..Default::default()
    //     };

    //     let mtt_table = MttTable::init_state(&mut effect, init_account)?;

    //     assert_eq!(mtt_table.table_id, 1);
    //     // assert_eq!(
    //     //     mtt_table.player_lookup.get(&1).map(|p| p.id),
    //     //     Some(&0)
    //     // );
    //     assert_eq!(effect.start_game, true);

    //     Ok(())
    // }

    // #[test]
    // fn test_checkpoint() -> anyhow::Result<()> {
    //     let data = [
    //         1, 0, 0, 0, 0, 0, 0, 0, 0, 32, 161, 7, 0, 0, 0, 0, 0, 64, 66, 15, 0, 0, 0, 0, 0, 0, 0,
    //         0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    //         0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 2, 0, 0, 0, 2, 0, 0, 0, 0, 0, 0,
    //         0, 2, 0, 0, 0, 0, 0, 0, 0, 224, 63, 238, 5, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    //         4, 0, 0, 0, 0, 0, 0, 0, 4, 0, 0, 0, 0, 0, 0, 0, 32, 130, 253, 5, 0, 0, 0, 0, 1, 0, 0,
    //         0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 2, 2, 0, 0, 0,
    //         0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    //         0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    //         0, 0, 0,
    //     ];
    //     let checkpoint = MttTableCheckpoint::try_from_slice(&data);
    //     println!("{:?}", checkpoint);
    //     Ok(())
    // }

    // #[test]
    // fn settle_should_emit_game_result() -> anyhow::Result<()> {
    //     let mut alice = TestClient::player("alice");
    //     let mut bob = TestClient::player("bob");
    //     let mut tx = TestClient::transactor("tx");
    //     let acc_builder = TestGameAccountBuilder::default()
    //         .add_player(&mut alice, 10000)
    //         .add_player(&mut bob, 10000)
    //         .set_transactor(&mut tx);

    //     let acc = acc_builder
    //         .with_data(InitTableData {
    //             table_id: 1,
    //             table_size: 6,
    //         })
    //         .build();
    //     let mut ctx = GameContext::try_new(&acc)?;
    //     let mut hdlr = TestHandler::<MttTable>::init_state(&mut ctx, &acc)?;
    //     hdlr.handle_dispatch_until_no_events(&mut ctx, vec![&mut tx])?;
    //     {
    //         let state = hdlr.get_state();
    //         assert_eq!(
    //             state.holdem.acting_player.as_ref().map(|a| a.id),
    //             Some(bob.id())
    //         );
    //         ctx.add_revealed_random(
    //             state.holdem.deck_random_id,
    //             HashMap::from([
    //                 (0, "ha".to_string()),
    //                 (1, "hk".to_string()),
    //                 (2, "h5".to_string()),
    //                 (3, "h6".to_string()),
    //                 (4, "h2".to_string()),
    //                 (5, "h4".to_string()),
    //                 (6, "h3".to_string()),
    //                 (7, "s3".to_string()),
    //                 (8, "s5".to_string()),
    //             ]),
    //         )?;
    //     }

    //     let evts = [
    //         bob.custom_event(GameEvent::Raise(10000)), // BTN/SB
    //         alice.custom_event(GameEvent::Call),       // BB
    //     ];

    //     for evt in evts {
    //         hdlr.handle_until_no_events(&mut ctx, &evt, vec![&mut tx])?;
    //     }

    //     {
    //         let state = hdlr.get_state();
    //         assert_eq!(state.holdem.street, Street::Showdown);
    //         // assert_eq!(
    //         //     ctx.get_bridge_events::<HoldemBridgeEvent>()?,
    //         //     vec![HoldemBridgeEvent::GameResult {
    //         //         table_id: 1,
    //         //         settles: vec![Settle::sub(alice.id(), 10000), Settle::add(bob.id(), 10000)],
    //         //         checkpoint: MttTableCheckpoint {
    //         //             btn: 1,
    //         //             players: vec![MttTablePlayer::new(1, 20000, 1)]
    //         //         }
    //         //     }]
    //         // );
    //     }

    //     Ok(())
    // }
}
