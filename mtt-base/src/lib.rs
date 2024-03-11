use borsh::{BorshDeserialize, BorshSerialize};
use race_api::event::BridgeEvent;
use race_api::prelude::*;

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
    pub table_id: u8,
    pub table_size: u8,
}

#[derive(Default, Debug, Clone, BorshSerialize, BorshDeserialize, PartialEq, Eq)]
pub struct MttTableCheckpoint {
    pub btn: usize,
    pub sb: u64,
    pub bb: u64,
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
    use borsh::BorshDeserialize;
    use crate::{MttTableCheckpoint, MttTablePlayer, InitTableData};

    #[test]
    fn test_parse_mtt_init_table_data() -> anyhow::Result<()> {
        let data = [207,128,234,18,142,1,0,0,0,225,245,5,0,0,0,0,2,32,161,7,0,0,0,0,0,96,234,0,0,0,0,0,0,0,0,0,0,3,0,0,0,50,30,20,0];
        let data = InitTableData::try_from_slice(&data);
        Ok(())
    }

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
