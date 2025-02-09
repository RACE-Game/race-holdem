use std::collections::BTreeMap;

use borsh::{BorshDeserialize, BorshSerialize};
use race_api::prelude::*;

#[derive(BorshSerialize, BorshDeserialize, Debug, PartialEq, Clone, Eq, Default)]
pub enum MttTablePlayerStatus {
    #[default]
    SitIn,
    SitOut,
}

#[derive(Debug, Clone, BorshSerialize, BorshDeserialize, Default, PartialEq, Eq)]
pub struct MttTablePlayer {
    pub id: u64,
    pub chips: u64,
    pub table_position: usize,
    pub player_status: MttTablePlayerStatus,
}

impl MttTablePlayer {
    pub fn new(id: u64, chips: u64, table_position: usize, player_status: MttTablePlayerStatus) -> Self {
        Self {
            id,
            chips,
            table_position,
            player_status,
        }
    }
}

#[derive(Default, Debug, Clone, BorshSerialize, BorshDeserialize, PartialEq, Eq)]
pub struct MttTableState {
    pub table_id: GameId,
    pub hand_id: usize,
    pub btn: usize,
    pub sb: u64,
    pub bb: u64,
    pub players: Vec<MttTablePlayer>,
    pub next_game_start: u64,
}

impl MttTableState {
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
            player_status: player.player_status.clone(),
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
    /// Start game with specified SB and BB.
    /// The `moved_players` indicates those should be removed before next hand.
    StartGame {
        sb: u64,
        bb: u64,
        moved_players: Vec<u64>,
    },
    /// Add players to current game.
    Relocate { players: Vec<MttTablePlayer> },
    /// Close table, all players should be removed from this game.
    /// Additionally, the game can be closed.
    CloseTable,
    /// A game result report from table.
    GameResult {
        hand_id: usize,
        table_id: GameId,
        chips_change: BTreeMap<u64, ChipsChange>,
        table: MttTableState,
    },
}

#[derive(Debug, BorshSerialize, BorshDeserialize, PartialEq, Eq, Clone)]
pub enum ChipsChange {
    Add(u64),
    Sub(u64),
}

impl BridgeEvent for HoldemBridgeEvent {}
