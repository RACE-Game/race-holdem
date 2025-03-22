use borsh::{BorshDeserialize, BorshSerialize};
use race_api::prelude::*;

#[derive(Debug, Clone, BorshSerialize, BorshDeserialize, Default, PartialEq, Eq)]
pub struct MttTablePlayer {
    pub id: u64,
    pub chips: u64,
    pub table_position: usize,
}

#[derive(Debug, Clone, BorshSerialize, BorshDeserialize, Default, PartialEq, Eq)]
pub struct MttTableSitin {
    pub id: u64,
    pub chips: u64,
}

impl MttTableSitin {
    pub fn new(id: u64, chips: u64) -> Self {
        Self { id, chips }
    }
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
    pub fn new(table_id: GameId, sb: u64, bb: u64, players: Vec<MttTablePlayer>) -> Self {
        Self {
            table_id,
            hand_id: 0,
            btn: 0,
            sb,
            bb,
            players,
            next_game_start: 0,
        }
    }

    pub fn find_position(&self) -> usize {
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
        return table_position;
    }

    /// If player already on table, returns false represents add failed.
    pub fn add_player(&mut self, player: &mut MttTablePlayer) -> bool {
        let exists = self.players.iter().any(|p| p.id == player.id);

        if exists {
            return false;
        } else {
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

            return true;
        }
    }

    /// Remove player on table, always return true as its an idempotent function.
    pub fn remove_player(&mut self, player_id: u64) -> bool {
        self.players.retain(|p| p.id != player_id);
        return true;
    }
}

#[derive(Debug, BorshSerialize, BorshDeserialize, PartialEq, Eq)]
pub enum PlayerResultStatus {
    Normal,
    Sitout,
    Eliminated,
}

#[derive(Debug, BorshSerialize, BorshDeserialize, PartialEq, Eq)]
pub struct PlayerResult {
    pub player_id: u64,
    pub chips: u64,
    pub chips_change: Option<ChipsChange>,
    pub status: PlayerResultStatus,
}

impl PlayerResult {
    pub fn new(player_id: u64, chips: u64, chips_change: Option<ChipsChange>, status: PlayerResultStatus) -> Self {
        Self {
            player_id, chips, chips_change, status
        }
    }
}

/// Holdem specific bridge events for interaction with the `mtt` crate.  Transactor will pass
/// through such events to the mtt handler.  Also see [`race_api::event::Event::Bridge`].
#[derive(Debug, BorshSerialize, BorshDeserialize, PartialEq, Eq)]
pub enum HoldemBridgeEvent {
    /// This event is sent by master game.
    /// Start game with specified SB and BB.
    /// The `moved_players` indicates those should be removed before next hand.
    StartGame {
        sb: u64,
        bb: u64,
        sitout_players: Vec<u64>,
    },
    /// This event is sent by master game.
    /// Add players to current game.
    SitinPlayers { sitins: Vec<MttTableSitin> },
    /// This event is sent by master game.
    /// Close table, all players should be removed from this game.
    /// Additionally, the game can be closed.
    CloseTable,
    /// This event is sent by sub game.
    /// A game result report from table.
    GameResult {
        hand_id: usize,
        table_id: GameId,
        player_results: Vec<PlayerResult>,
        table: MttTableState,
    },
}

#[derive(Debug, BorshSerialize, BorshDeserialize, PartialEq, Eq, Clone)]
pub enum ChipsChange {
    Add(u64),
    Sub(u64),
}

impl BridgeEvent for HoldemBridgeEvent {}
