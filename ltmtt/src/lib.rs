use borsh::{BorshDeserialize, BorshSerialize};
// use race_api::engine::GameHandler;
use race_api::prelude::*;
use race_holdem_mtt_base::{ChipsChange, HoldemBridgeEvent, MttTablePlayer, MttTableState};
use std::{collections::BTreeMap, f32::consts::E};

type PlayerId = u64;

#[derive(Default, BorshSerialize, BorshDeserialize, Debug, Clone, Copy)]
pub enum LtMttStage {
    #[default]
    Init,
    Playing,
    EntryClosed,
    Completed,
}

#[derive(BorshSerialize, BorshDeserialize, Debug, PartialEq, Eq)]
pub struct BlindRuleItem {
    sb_x: u16,
    bb_x: u16,
}

#[derive(BorshSerialize, Default, BorshDeserialize, Debug, PartialEq, Eq)]
pub struct BlindInfo {
    blind_base: u64,
    blind_interval: u64,
    blind_rules: Vec<BlindRuleItem>,
}

#[derive(Default, BorshSerialize, BorshDeserialize)]
pub struct LtMttAccountData {
    start_time: u64,
    ticket: u64,
    table_size: u8,
    start_chips: u64,
    // blind_info: BlindInfo,
    prize_rules: Vec<u8>,
    theme: Option<String>, // optional NFT theme
    subgame_bundle: String,
}
#[derive(Debug, BorshSerialize, BorshDeserialize, Default)]
pub struct Player {
    id: PlayerId,
    chips: u64,
}

// impl Ord for Player {
//     fn cmp(&self, other: &Self) -> std::cmp::Ordering {
//         self.chips.cmp(&other.chips).reverse()
//     }
// }

#[derive(Default, BorshSerialize, BorshDeserialize, Debug)]
pub struct LtMtt {
    start_time: u64,
    stage: LtMttStage,
    ticket: u64,
    table_size: u8,
    start_chips: u64,
    prize_rules: Vec<u8>,
    theme: Option<String>,
    subgame_bundle: String,
    tables: BTreeMap<u8, MttTableState>,
    table_assigns: BTreeMap<PlayerId, u8>,
    rankings: Vec<Player>,
}
impl GameHandler for LtMtt {
    fn init_state(init_account: InitAccount) -> HandleResult<Self> {
        let LtMttAccountData {
            start_time,
            ticket,
            table_size,
            start_chips,
            // mut blind_info,
            prize_rules,
            theme,
            subgame_bundle,
        } = init_account.data()?;

        let state = Self {
            start_time,
            ticket,
            table_size,
            start_chips,
            // blind_info,
            prize_rules,
            theme,
            subgame_bundle,
            ..Default::default()
        };

        Ok(state)
    }

    fn handle_event(&mut self, effect: &mut Effect, event: Event) -> HandleResult<()> {
        match event {
            Event::Ready => {
                self.stage = LtMttStage::EntryClosed;
            }

            Event::Join { players } => {
                self.stage = LtMttStage::Playing;
                for player in players {
                    self.rankings.push(Player {
                        id: player.id(),
                        chips: self.start_chips,
                    });
                }
            }

            _ => {}
        }
        effect.wait_timeout(5000);
        Ok(())
    }
}

impl LtMtt {
    // implement some crud methods here
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_join() -> anyhow::Result<()> {
        let mut state = LtMtt::default();
        let mut effect = Effect::default();
        let event = Event::Join {
            players: vec![GamePlayer::new(1, 100)],
        };
        state.handle_event(&mut effect, event)?;
        assert_eq!(state.rankings.len(), 1);
        assert_eq!(effect.wait_timeout, Some(5000));
        
        Ok(())
    }
}
