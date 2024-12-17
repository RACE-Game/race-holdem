mod account_data;
mod player;
mod errors;

use std::collections::BTreeMap;

use crate::account_data::LtMttAccountData;
use crate::player::{Player, PlayerStatus, Ranking};
use crate::errors::error_leave_not_allowed;


use borsh::{BorshDeserialize, BorshSerialize};
// use race_api::engine::GameHandler;
use race_api::{prelude::*, types::EntryLock, types::GameDeposit};
use race_holdem_mtt_base::{HoldemBridgeEvent, MttTablePlayer, MttTableState};
use race_proc_macro::game_handler;

type Millis = u64;
type PlayerId = u64;

#[derive(Default, PartialEq, BorshSerialize, BorshDeserialize, Debug, Clone, Copy)]
pub enum LtMttStage {
    #[default]
    Init,
    EntryOpened,
    EntryClosed,
    Settled,
}

#[derive(BorshSerialize, BorshDeserialize, Debug, PartialEq, Clone)]
pub enum ClientEvent {
    SitIn,
    SitOut,
}

impl CustomEvent for ClientEvent {}

#[game_handler]
#[derive(Default, BorshSerialize, BorshDeserialize, Debug)]
pub struct LtMtt {
    entry_start_time: Millis,
    entry_close_time: Millis,
    settle_time: Millis,
    table_size: u8,
    ticket: u64,
    start_chips: u64,
    //
    players: BTreeMap<PlayerId, Player>,
    rankings: Vec<Ranking>,
    total_prize: u64,
    stage: LtMttStage,
    tables: BTreeMap<u8, MttTableState>,
    table_assigns: BTreeMap<PlayerId, u8>,
    subgame_bundle: String,
    prize_rules: Vec<u8>,
    // rake: u64,
    // blind_info: BlindInfo,
    // theme: Option<String>,
}

impl GameHandler for LtMtt {
    fn init_state(init_account: InitAccount) -> HandleResult<Self> {
        let LtMttAccountData {
            entry_start_time,
            entry_close_time,
            settle_time,
            table_size,
            ticket,
            start_chips,
        } = init_account.data()?;

        let state = Self {
            entry_start_time,
            entry_close_time,
            settle_time,
            table_size,
            ticket,
            start_chips,
            ..Default::default()
        };

        Ok(state)
    }

    fn handle_event(&mut self, effect: &mut Effect, event: Event) -> HandleResult<()> {
        match event {
            Event::Ready => self.on_ready(effect)?,
            Event::GameStart => self.on_game_start(effect)?,
            Event::WaitingTimeout => self.on_waiting_timeout(effect)?,
            Event::Join { players } => self.on_join(effect, players)?,
            Event::Deposit { deposits } => self.on_deposit(deposits)?,
            _ => (),
        }

        Ok(())
    }
}

impl LtMtt {
    fn on_ready(&self, effect: &mut Effect) -> HandleResult<()> {
        if effect.timestamp() > self.entry_start_time
            || self.entry_start_time > self.entry_close_time
            || self.entry_close_time > self.settle_time
        {
            effect.set_entry_lock(EntryLock::Closed);
        } else {
            effect.start_game();
        }

        Ok(())
    }

    fn on_game_start(&mut self, effect: &mut Effect) -> HandleResult<()> {
        if self.entry_start_time > effect.timestamp() {
            effect.wait_timeout(self.entry_start_time - effect.timestamp());
        } else {
            effect.set_entry_lock(EntryLock::Closed);
        }

        Ok(())
    }

    fn on_waiting_timeout(&mut self, effect: &mut Effect) -> HandleResult<()> {
        match self.stage {
            LtMttStage::Init => {
                self.stage = LtMttStage::EntryOpened;
                effect.wait_timeout(self.entry_close_time - effect.timestamp());
            }

            LtMttStage::EntryOpened => {
                self.stage = LtMttStage::EntryClosed;
                effect.set_entry_lock(EntryLock::Closed);
                effect.wait_timeout(self.settle_time - effect.timestamp());
            }

            LtMttStage::EntryClosed => {
                self.stage = LtMttStage::Settled;
                self.do_settle(effect)?;
            }

            _ => {}
        }
        Ok(())
    }

    fn on_join(&mut self, effect: &mut Effect, new_players: Vec<GamePlayer>) -> HandleResult<()> {
        if self.stage == LtMttStage::EntryOpened {
            for player in new_players {
                self.players.insert(
                    player.id(),
                    Player {
                        player_id: player.id(),
                        position: player.position(),
                        status: PlayerStatus::SatIn,
                    },
                );
                self.do_sit_in(effect, player.id())?;
            }
        }

        Ok(())
    }

    fn on_deposit(&mut self, deposits: Vec<GameDeposit>) -> HandleResult<()> {
        if self.stage == LtMttStage::EntryOpened {
            for deposit in deposits {
                self.total_prize += deposit.balance();

                if let Some(rank) = self
                    .rankings
                    .iter_mut()
                    .find(|r| r.player_id == deposit.id())
                {
                    rank.chips = self.start_chips;
                    rank.deposit_history.push(deposit.balance());
                } else {
                    self.rankings.push(Ranking {
                        player_id: deposit.id(),
                        chips: self.start_chips,
                        deposit_history: vec![deposit.balance()],
                    })
                }
            }
        }

        Ok(())
    }

    fn do_sit_in(&mut self, effect: &mut Effect, player_id: PlayerId) -> HandleResult<()> {
        let table_id = self.find_table_sit_in();

        if table_id == 0 {
            // no availiable table found
            let new_table_id = self.create_table(effect)?;
            self.table_assigns.insert(player_id, new_table_id);

            let table_ref = self
                .tables
                .get_mut(&new_table_id)
                .ok_or(errors::error_table_not_found())?;

            let mut mtt_table_player = MttTablePlayer::new(player_id, self.start_chips, 0);
            table_ref.add_player(&mut mtt_table_player);

            effect.bridge_event(
                new_table_id as _,
                HoldemBridgeEvent::Relocate {
                    players: vec![mtt_table_player],
                },
            );
        } else {
            self.table_assigns.insert(player_id, table_id);

            let table_ref = self
                .tables
                .get_mut(&table_id)
                .ok_or(errors::error_table_not_found())?;

            let mut mtt_table_player = MttTablePlayer::new(player_id, self.start_chips, 0);
            table_ref.add_player(&mut mtt_table_player);

            effect.bridge_event(
                table_id as _,
                HoldemBridgeEvent::Relocate {
                    players: vec![mtt_table_player],
                },
            );
        }

        effect.checkpoint();
        Ok(())
    }

    fn do_settle(&mut self, effect: &mut Effect) -> HandleResult<()> {
        let total_shares: u8 = self.prize_rules.iter().take(self.rankings.len()).sum();
        let prize_share: u64 = self.total_prize / total_shares as u64;

        for (i, ranking) in self.rankings.iter().enumerate() {
            let player_id = ranking.player_id;

            if let Some(rank) = self.prize_rules.get(i) {
                let prize: u64 = prize_share * *rank as u64;
                effect.settle(player_id, prize, false)?;
            }
        }

        Ok(())
    }

    fn find_table_sit_in(&self) -> u8 {
        let mut min = self.table_size;

        // After iteration, it will be the table with the least players,
        // or 0 represents no table or all tables are full.
        self.tables.iter().fold(0, |table_id, (id, table)| {
            if (table.players.len() as u8) < min {
                min = table.players.len() as u8;
                *id
            } else {
                table_id
            }
        })
    }

    fn create_table(&mut self, effect: &mut Effect) -> HandleResult<u8> {
        let table_id = self.tables.iter().map(|(id, _)| *id).max().unwrap_or(0) + 1;
        let sb = 1000 as u64;
        let bb = 2000 as u64;

        let table = MttTableState {
            table_id,
            btn: 0,
            sb,
            bb,
            players: Vec::new(),
            next_game_start: 0,
            hand_id: 0,
        };

        effect.launch_sub_game(
            table_id as _,
            self.subgame_bundle.clone(),
            self.table_size as _,
            &table,
        )?;
        self.tables.insert(table_id, table);

        Ok(table_id)
    }
}

#[cfg(test)]
mod tests {
    // use std::time::SystemTime;
    // use super::*;
    // use race_test::prelude::*;

    // #[test]
    // fn test_join() -> anyhow::Result<()> {
    //     let mut state = LtMtt::default();
    //     let mut effect = Effect::default();
    //     let event = Event::Join {
    //         players: vec![GamePlayer::new(1, 100)],
    //     };
    //     state.handle_event(&mut effect, event)?;
    //     assert_eq!(state.rankings.len(), 1);
    //     assert_eq!(effect.wait_timeout, Some(5000));

    //     Ok(())
    // }

    // #[test]
    // fn test_with_account() -> anyhow::Result<()> {
    //     let mut transactor = TestClient::transactor("server");
    //     let mut alice = TestClient::player("Alice");
    //     // let mut bob = TestClient::player("Bob");
    //     // alice.custom_event(custom_event::Game::Ready);
    //     let ts = SystemTime::now()
    //         .duration_since(SystemTime::UNIX_EPOCH)?
    //         .as_millis() as u64;

    //     let (mut context, _) = TestContextBuilder::default()
    //         .set_transactor(&mut transactor)
    //         .with_data(LtMttAccountData {
    //             start_time: ts,
    //             end_time: ts + 60 * 60 * 1000,
    //             table_size: 2,
    //             start_chips: 100,
    //             prize_rules: vec![100, 50, 30],
    //             theme: None,
    //             subgame_bundle: "subgame".to_string(),
    //         })
    //         .with_max_players(10)
    //         .build_with_init_state::<LtMtt>()?;

    //     let event = context.join(&mut alice, 100);
    //     context.handle_event(&event)?;

    //     {
    //         let state = context.state();
    //         assert_eq!(state.rankings.len(), 1);
    //     }

    //     Ok(())
    // }
}
