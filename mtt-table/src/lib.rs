mod errors;

use borsh::{BorshDeserialize, BorshSerialize};
use race_api::prelude::*;
use race_api::event::BridgeEvent;
use race_holdem_base::essential::{GameMode, Player};
use race_holdem_base::game::Holdem;
use race_proc_macro::game_handler;
use std::collections::BTreeMap;
use race_holdem_mtt_base::{MttTablePlayer, InitTableData, MttTableCheckpoint, HoldemBridgeEvent, GameResult};

#[game_handler]
#[derive(BorshSerialize, BorshDeserialize, Default)]
pub struct MttTable {
    pub table_id: u8,
    pub holdem: Holdem,
    pub player_lookup: BTreeMap<u16, Player>, // The mapping from players' game position to player struct
}

impl GameHandler for MttTable {
    type Checkpoint = MttTableCheckpoint;

    fn init_state(effect: &mut Effect, init_account: InitAccount) -> HandleResult<Self> {
        let InitTableData {
            btn,
            player_lookup,
            table_id,
            sb,
            bb,
            table_size,
        } = init_account.data()?;
        let mut player_map = BTreeMap::new();

        for (_, player) in player_lookup.clone() {
            player_map.insert(player.addr(), player);
        }

        let holdem = Holdem {
            btn,
            sb,
            bb,
            player_map,
            table_size,
            mode: GameMode::Mtt,
            ..Default::default()
        };

        effect.start_game();

        Ok(Self {
            table_id,
            player_lookup,
            holdem,
        })
    }

    fn handle_event(&mut self, effect: &mut Effect, event: Event) -> HandleResult<()> {
        match event {
            Event::Bridge { dest, raw } => {
                if dest == self.table_id as _ {
                    let bridge_event = HoldemBridgeEvent::try_parse(&raw)?;
                    self.handle_bridge_event(effect, bridge_event)?;
                }
            }
            _ => self.holdem.handle_event(effect, event)?,
        };

        // Check if there's a settlement
        if effect.is_checkpoint {
            let settles = effect.settles.clone();
            let checkpoint = self.derive_checkpoint()?;
            let evt = HoldemBridgeEvent::GameResult {
                checkpoint,
                settles,
                table_id: self.table_id,
            };
            effect.bridge_event(evt)?;
        }
        Ok(())
    }

    fn into_checkpoint(self) -> HandleResult<Self::Checkpoint> {
        self.derive_checkpoint()
    }
}

impl MttTable {
    fn derive_checkpoint(&self) -> HandleResult<MttTableCheckpoint> {
        let mut players = vec![];
        for (addr, player) in self.holdem.player_map.iter() {
            let mtt_position = self
                .find_game_position(&addr)
                .ok_or(errors::internal_player_position_missing())?;
            players.push(MttTablePlayer {
                mtt_position,
                chips: player.chips,
                table_position: player.position,
            })
        }
        Ok(MttTableCheckpoint {
            btn: self.holdem.btn,
            players,
        })
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
                checkpoint,
                player_lookup,
            } => {
                self.holdem.sb = sb;
                self.holdem.bb = bb;
                self.player_lookup = player_lookup;
                self.holdem.player_map.clear();
                for p in checkpoint.players {
                    let addr = self
                        .find_player_addr(p.mtt_position)
                        .ok_or(errors::internal_player_addr_missing())?;
                    self.holdem.player_map.insert(
                        addr.clone(),
                        Player::new(addr, p.chips, p.table_position as _, 0),
                    );
                }
                effect.start_game();
            }
            _ => return Err(errors::internal_invalid_bridge_event()),
        };
        Ok(())
    }

    fn find_player_addr(&self, game_position: u16) -> Option<String> {
        self.player_lookup
            .get(&game_position)
            .map(|p| p.addr.clone())
    }

    fn find_game_position(&self, addr: &str) -> Option<u16> {
        self.player_lookup
            .iter()
            .find(|(_, player)| player.addr.eq(&addr))
            .map(|(pos, _)| *pos)
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;
    use race_holdem_base::essential::{GameEvent, Street};
    use race_test::prelude::*;

    #[test]
    fn test_init_state() -> anyhow::Result<()> {
        let mut effect = Effect::default();
        let init_account = InitAccount {
            data: InitTableData {
                btn: 0,
                table_id: 1,
                sb: 50,
                bb: 100,
                table_size: 6,
                player_lookup: BTreeMap::from([
                    (1, Player::new("alice", 10000, 0, 0)),
                    (2, Player::new("bob", 10000, 0, 0)),
                ]),
            }
            .try_to_vec()?,
            ..Default::default()
        };

        let mtt_table = MttTable::init_state(&mut effect, init_account)?;

        assert_eq!(mtt_table.table_id, 1);
        assert_eq!(
            mtt_table.player_lookup.get(&1).map(|p| p.addr.as_str()),
            Some("alice")
        );
        assert_eq!(effect.start_game, true);

        Ok(())
    }

    #[test]
    fn settle_should_emit_game_result() -> anyhow::Result<()> {
        let alice = TestClient::player("alice");
        let bob = TestClient::player("bob");
        let mut tx = TestClient::transactor("tx");
        let player_lookup = BTreeMap::from([
            (0, Player::new("alice", 10000, 0, 0)),
            (1, Player::new("bob", 10000, 1, 0)),
        ]);
        let acc = TestGameAccountBuilder::default()
            .add_player(&alice, 10000)
            .add_player(&bob, 10000)
            .set_transactor(&tx)
            .with_data(InitTableData {
                sb: 10,
                bb: 20,
                btn: 0,
                table_id: 1,
                table_size: 6,
                player_lookup,
            })
            .build();
        let mut ctx = GameContext::try_new(&acc)?;
        let mut hdlr = TestHandler::<MttTable>::init_state(&mut ctx, &acc)?;
        hdlr.handle_dispatch_until_no_events(&mut ctx, vec![&mut tx])?;
        {
            let state = hdlr.get_state();
            assert_eq!(
                state.holdem.acting_player.as_ref().map(|a| a.addr.as_str()),
                Some("bob")
            );
            ctx.add_revealed_random(
                state.holdem.deck_random_id,
                HashMap::from([
                    (0, "ha".to_string()),
                    (1, "hk".to_string()),
                    (2, "h5".to_string()),
                    (3, "h6".to_string()),
                    (4, "h2".to_string()),
                    (5, "h4".to_string()),
                    (6, "h3".to_string()),
                    (7, "s3".to_string()),
                    (8, "s5".to_string()),
                ]),
            )?;
        }

        let evts = [
            bob.custom_event(GameEvent::Raise(10000)), // BTN/SB
            alice.custom_event(GameEvent::Call),       // BB
        ];

        for evt in evts {
            hdlr.handle_until_no_events(&mut ctx, &evt, vec![&mut tx])?;
        }

        {
            let state = hdlr.get_state();
            assert_eq!(state.holdem.street, Street::Showdown);
            assert_eq!(
                ctx.get_bridge_events::<HoldemBridgeEvent>()?,
                vec![HoldemBridgeEvent::GameResult {
                    table_id: 1,
                    settles: vec![Settle::sub("alice", 10000), Settle::add("bob", 10000)],
                    checkpoint: MttTableCheckpoint {
                        btn: 1,
                        players: vec![MttTablePlayer::new(1, 20000, 1)]
                    }

                }]
            );
        }

        Ok(())
    }
}
