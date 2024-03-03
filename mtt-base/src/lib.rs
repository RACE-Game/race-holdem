use borsh::{BorshDeserialize, BorshSerialize};
use race_api::event::BridgeEvent;
use race_api::prelude::*;
use race_holdem_base::essential::Player;
use std::collections::BTreeMap;

type PlayerId = u64;

#[derive(Debug, Clone, BorshSerialize, BorshDeserialize, Default, PartialEq, Eq)]
pub struct MttTablePlayer {
    pub id: u64,
    pub chips: u64,
    pub table_position: usize,
}

impl MttTablePlayer {
    pub fn new(id: u64, chips: u64, table_position: usize) -> Self {
        Self {
            id,
            chips,
            table_position,
        }
    }
}


#[derive(BorshSerialize, BorshDeserialize, Default, Debug)]
pub struct InitTableData {
    pub btn: usize,
    pub table_id: u8,
    pub sb: u64,
    pub bb: u64,
    pub table_size: u8,
    pub player_lookup: BTreeMap<PlayerId, Player>,
}

#[derive(Default, Debug, Clone, BorshSerialize, BorshDeserialize, PartialEq, Eq)]
pub struct MttTableCheckpoint {
    pub btn: usize,
    pub players: Vec<MttTablePlayer>,
}

impl MttTableCheckpoint {
    pub fn add_player(&mut self, player: &mut MttTablePlayer) {
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
            id: player.id,
            chips: player.chips,
            table_position,
        });
        // Update relocated player's table position as well
        player.table_position = table_position;
    }
}

/// Holdem specific bridge events for interaction with the `mtt` crate.  Transactor will pass
/// through such events to the mtt handler.  Also see [`race_api::event::Event::Bridge`].
#[derive(Debug, BorshSerialize, BorshDeserialize, PartialEq, Eq)]
pub enum HoldemBridgeEvent {
    StartGame {
        sb: u64,
        bb: u64,
        moved_players: Vec<u64>,
    },
    Relocate {
        players: Vec<MttTablePlayer>,
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
                    id: 0,
                    chips: 100,
                    table_position: 0,
                },
                MttTablePlayer {
                    id: 1,
                    chips: 100,
                    table_position: 1,
                },
                MttTablePlayer {
                    id: 2,
                    chips: 100,
                    table_position: 3,
                },
            ],
        };
        table.add_player(&mut MttTablePlayer { id: 4, chips: 100, table_position: 0 });

        assert_eq!(table.players.len(), 4);
        assert_eq!(table.players[3].table_position, 2);
        assert_eq!(table.players[3].id, 4);
        Ok(())
    }
}
