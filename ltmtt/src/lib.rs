mod account_data;
mod player;

use crate::account_data::LtMttAccountData;
use crate::player::LtMttPlayer;

use borsh::{BorshDeserialize, BorshSerialize};
// use race_api::engine::GameHandler;
use race_api::{prelude::*, types::EntryLock};

type Millis = u64;

#[derive(Default, PartialEq, BorshSerialize, BorshDeserialize, Debug, Clone, Copy)]
pub enum LtMttStage {
    #[default]
    Init,
    EntryOpened,
    EntryClosed,
    Settled,
}

#[derive(Default, BorshSerialize, BorshDeserialize, Debug)]
pub struct LtMtt {
    entry_start_time: Millis,
    entry_close_time: Millis,
    settle_time: Millis,
    stage: LtMttStage,
    table_size: u8,
    start_chips: u64,
    rankings: Vec<LtMttPlayer>,
    // rake: u64,
    // blind_info: BlindInfo,
    // prize_rules: Vec<u8>,
    // theme: Option<String>,
    // subgame_bundle: String,
    // tables: BTreeMap<u8, MttTableState>,
    // table_assigns: BTreeMap<PlayerId, u8>,
}

impl GameHandler for LtMtt {
    fn init_state(init_account: InitAccount) -> HandleResult<Self> {
        let LtMttAccountData {
            entry_start_time,
            entry_close_time,
            settle_time,
            table_size,
            start_chips,
        } = init_account.data()?;

        let state = Self {
            entry_start_time,
            entry_close_time,
            settle_time,
            table_size,
            start_chips,
            ..Default::default()
        };

        Ok(state)
    }

    fn handle_event(&mut self, effect: &mut Effect, event: Event) -> HandleResult<()> {
        match event {
            Event::Ready => self.on_ready(effect)?,
            Event::GameStart => (),
            Event::WaitingTimeout => self.on_waiting_timeout(effect)?,
            Event::Join { players } => self.on_join(players)?,
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
            effect.wait_timeout(self.entry_start_time - effect.timestamp());
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
                self.settle(effect)?;
            }

            _ => {}
        }
        Ok(())
    }

    fn on_join(&mut self, new_players: Vec<GamePlayer>) -> HandleResult<()> {
        if self.stage == LtMttStage::EntryOpened {
            for player in new_players {
                self.rankings.push(LtMttPlayer {
                    id: player.id(),
                    position: player.position(),
                    chips: self.start_chips,
                });
            }
        }

        Ok(())
    }

    fn settle(&mut self, effect: &mut Effect) -> HandleResult<()> {
        effect.settle(0, 0)?;
        Ok(())
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
