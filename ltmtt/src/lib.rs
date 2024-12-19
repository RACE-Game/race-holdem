mod errors;
mod player;

use std::collections::BTreeMap;
use std::vec;

// use crate::errors::error_leave_not_allowed;
use crate::player::{Player, PlayerStatus, Ranking};

use borsh::{BorshDeserialize, BorshSerialize};
// use race_api::engine::GameHandler;
use race_api::{prelude::*, types::EntryLock, types::GameDeposit};
use race_holdem_mtt_base::{HoldemBridgeEvent, MttTablePlayer, MttTableState};
use race_proc_macro::game_handler;

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

#[derive(Default, Debug, BorshSerialize, BorshDeserialize, Clone)]
pub struct TicketRule {
    times: Option<usize>,
    amount: u64,
    chips: u64,
}

#[derive(Default, BorshSerialize, BorshDeserialize)]
pub struct LtMttAccountData {
    pub entry_open_time: u64,
    pub entry_close_time: u64,
    pub settle_time: u64,
    pub table_size: u8,
    pub ticket_rules: Vec<TicketRule>,
    // blind_info: BlindInfo,
    // prize_rules: Vec<u8>,
    // theme: Option<String>, // optional NFT theme
    // subgame_bundle: String,
}

impl CustomEvent for ClientEvent {}

#[game_handler]
#[derive(Default, BorshSerialize, BorshDeserialize, Debug)]
pub struct LtMtt {
    // time unit is millis
    entry_open_time: u64,
    entry_close_time: u64,
    settle_time: u64,
    // the maximum number is 9 players at the same time,
    // but never check in ltmtt, it's the launcher's responsibility.
    table_size: u8,
    ticket_rules: Vec<TicketRule>,
    /// belows are ltmtt self hold fields
    //
    players: BTreeMap<u64, Player>,
    rankings: Vec<Ranking>,
    total_prize: u64,
    stage: LtMttStage,
    tables: BTreeMap<usize, MttTableState>,
    table_assigns: BTreeMap<u64, usize>,
    subgame_bundle: String,
    prize_rules: Vec<u8>,
    // rake: u64,
    // blind_info: BlindInfo,
    // theme: Option<String>,
}

impl GameHandler for LtMtt {
    fn init_state(init_account: InitAccount) -> HandleResult<Self> {
        let LtMttAccountData {
            entry_open_time,
            entry_close_time,
            settle_time,
            table_size,
            ticket_rules,
        } = init_account.data()?;

        // If params invalid, avoid create LtMtt game.
        if entry_open_time > entry_close_time || entry_close_time > settle_time {
            return Err(HandleError::MalformedGameAccountData);
        }

        let state = Self {
            entry_open_time,
            entry_close_time,
            settle_time,
            table_size,
            ticket_rules,
            ..Default::default()
        };

        Ok(state)
    }

    fn handle_event(&mut self, effect: &mut Effect, event: Event) -> HandleResult<()> {
        match event {
            Event::Ready => self.on_ready(effect)?,
            Event::GameStart => self.on_game_start(effect)?,
            Event::WaitingTimeout => self.on_waiting_timeout(effect)?,
            // Only first register will receive this event.
            Event::Join { players } => self.on_join(effect, players)?,
            // If player loss all chips, he can register again, then ltmtt will receive deposit event.
            Event::Deposit { deposits } => self.on_deposit(effect, deposits)?,
            _ => (),
        }

        Ok(())
    }
}

impl LtMtt {
    fn on_ready(&self, effect: &mut Effect) -> HandleResult<()> {
        effect.info("callback on_ready...");
        effect.set_entry_lock(EntryLock::Closed);
        effect.start_game();

        Ok(())
    }

    // reset game state, swap secret
    fn on_game_start(&mut self, effect: &mut Effect) -> HandleResult<()> {
        effect.info("callback on_game_start...");
        effect.wait_timeout(self.entry_open_time - effect.timestamp());

        Ok(())
    }

    fn on_waiting_timeout(&mut self, effect: &mut Effect) -> HandleResult<()> {
        match self.stage {
            LtMttStage::Init => {
                effect.info("callback on_waiting_timeout: stage changed to EntryOpened.");
                effect.wait_timeout(self.entry_close_time - effect.timestamp());
                effect.set_entry_lock(EntryLock::Open);
                self.stage = LtMttStage::EntryOpened;
            }

            LtMttStage::EntryOpened => {
                effect.info("callback on_waiting_timeout: stage changed to EntryClosed.");
                effect.wait_timeout(self.settle_time - effect.timestamp());
                effect.set_entry_lock(EntryLock::Closed);
                self.stage = LtMttStage::EntryClosed;
            }

            LtMttStage::EntryClosed => {
                effect.info("callback on_waiting_timeout: stage changed to Settled.");
                self.do_settle(effect)?;
                self.stage = LtMttStage::Settled;
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

    fn on_deposit(&mut self, effect: &mut Effect, deposits: Vec<GameDeposit>) -> HandleResult<()> {
        if self.stage == LtMttStage::EntryOpened {
            for deposit in deposits {
                if let Some(rank) = self
                    .rankings
                    .iter_mut()
                    .find(|r| r.player_id == deposit.id())
                {
                    self.total_prize += deposit.balance();
                    rank.chips = self.start_chips;
                    rank.deposit_history.push(deposit.balance());
                    effect.accept_deposit(&deposit)?;
                } else {
                    effect.reject_deposit(&deposit)?;
                }
            }
        }

        Ok(())
    }

    fn do_sit_in(&mut self, effect: &mut Effect, player_id: u64) -> HandleResult<()> {
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
            )?;
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
            )?;
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

    fn find_table_sit_in(&self) -> usize {
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

    fn create_table(&mut self, effect: &mut Effect) -> HandleResult<usize> {
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

        effect.launch_sub_game(self.subgame_bundle.clone(), self.table_size as _, &table)?;
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
