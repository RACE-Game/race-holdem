use borsh::{BorshDeserialize, BorshSerialize};
use race_api::event::BridgeEvent;
use race_api::prelude::*;
use race_holdem_base::essential::Player;
use std::collections::BTreeMap;

#[derive(Debug, BorshSerialize, BorshDeserialize, Default, PartialEq, Eq)]
pub struct MttTablePlayer {
    pub mtt_position: u16,
    pub chips: u64,
    pub table_position: usize,
}

impl MttTablePlayer {
    pub fn new(mtt_position: u16, chips: u64, table_position: usize) -> Self {
        Self {
            mtt_position,
            chips,
            table_position,
        }
    }
}

#[derive(BorshSerialize, BorshDeserialize, Default)]
pub struct InitTableData {
    pub btn: usize,
    pub table_id: u8,
    pub sb: u64,
    pub bb: u64,
    pub table_size: u8,
    pub player_lookup: BTreeMap<u16, Player>,
}

#[derive(Debug, BorshSerialize, BorshDeserialize, Default, PartialEq, Eq)]
pub struct MttTableCheckpoint {
    pub btn: usize,
    pub players: Vec<MttTablePlayer>,
}

impl MttTableCheckpoint {
    pub fn add_player(&mut self, mtt_position: u16, chips: u64) -> usize {
        let mut table_position = 0;
        for i in 0.. {
            if self
                .players
                .iter()
                .find(|p| p.table_position == i)
                .is_none()
            {
                table_position = i;
                break;
            }
        }
        self.players.push(MttTablePlayer {
            mtt_position,
            chips,
            table_position,
        });
        table_position
    }
}

/// Holdem specific bridge events for interaction with the `mtt` crate.  Transactor will pass
/// through such events to the mtt handler.  Also see [`race_api::event::Event::Bridge`].
#[derive(Debug, BorshSerialize, BorshDeserialize, PartialEq, Eq)]
pub enum HoldemBridgeEvent {
    StartGame {
        sb: u64,
        bb: u64,
        moved_players: Vec<u16>,
    },
    AddPlayers {
        player_lookup: BTreeMap<u16, Player>,
    },
    CloseTable,
    GameResult {
        table_id: u8,
        settles: Vec<Settle>,
        checkpoint: MttTableCheckpoint,
    },
}

impl BridgeEvent for HoldemBridgeEvent {}

#[cfg(test)]
mod tests {
    use crate::{MttTableCheckpoint, MttTablePlayer};

    #[test]
    fn test_add_player_to_table() -> anyhow::Result<()> {
        let mut table = MttTableCheckpoint {
            btn: 0,
            players: vec![
                MttTablePlayer {
                    mtt_position: 0,
                    chips: 100,
                    table_position: 0,
                },
                MttTablePlayer {
                    mtt_position: 1,
                    chips: 100,
                    table_position: 1,
                },
                MttTablePlayer {
                    mtt_position: 2,
                    chips: 100,
                    table_position: 3,
                },
            ],
        };
        let pos = table.add_player(4, 100);

        assert_eq!(pos, 2);
        assert_eq!(table.players.len(), 4);
        assert_eq!(table.players[3].table_position, 2);
        assert_eq!(table.players[3].mtt_position, 4);
        Ok(())
    }
}
