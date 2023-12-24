use borsh::{BorshDeserialize, BorshSerialize};
use race_api::prelude::*;
use race_api::event::BridgeEvent;
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

/// Holdem specific bridge events for interaction with the `mtt` crate.  Transactor will pass
/// through such events to the mtt handler.  Also see [`race_api::event::Event::Bridge`].
#[derive(Debug, BorshSerialize, BorshDeserialize, PartialEq, Eq)]
pub enum HoldemBridgeEvent {
    StartGame {
        sb: u64,
        bb: u64,
        checkpoint: MttTableCheckpoint,
        player_lookup: BTreeMap<u16, Player>,
    },
    GameResult {
        table_id: u8,
        settles: Vec<Settle>,
        checkpoint: MttTableCheckpoint,
    },
}

impl BridgeEvent for HoldemBridgeEvent {}

#[derive(BorshSerialize, BorshDeserialize, Debug)]
pub struct GameResult {
    pub table_id: usize,
    pub settles: Vec<Settle>,
}
