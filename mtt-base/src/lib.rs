use borsh::{BorshDeserialize, BorshSerialize};
use race_api::prelude::*;

pub const DEFAULT_TIME_CARDS: u8 = 5;

#[derive(Debug, Clone, BorshSerialize, BorshDeserialize, Default, PartialEq, Eq)]
pub struct MttTablePlayer {
    pub id: u64,
    pub chips: u64,
    pub time_cards: u8,
}

impl MttTablePlayer {
    pub fn new(id: u64, chips: u64, time_cards: u8) -> Self {
        Self {
            id,
            chips,
            time_cards,
        }
    }

    /// Give time_cards a default value.
    /// For testing
    pub fn new_with_defaults(id: u64, chips: u64) -> Self {
        Self::new(id, chips, DEFAULT_TIME_CARDS)
    }
}

#[derive(Debug, Clone, BorshSerialize, BorshDeserialize, Default, PartialEq, Eq)]
pub struct MttTableSitin {
    pub id: u64,
    pub chips: u64,
    pub time_cards: u8,
}

impl MttTableSitin {
    pub fn new(id: u64, chips: u64, time_cards: u8) -> Self {
        Self {
            id,
            chips,
            time_cards,
        }
    }

    /// Give time_cards a default value.
    /// For testing
    pub fn new_with_defaults(id: u64, chips: u64) -> Self {
        Self::new(id, chips, DEFAULT_TIME_CARDS)
    }
}

#[derive(Default, Debug, Clone, BorshSerialize, BorshDeserialize, PartialEq, Eq)]
pub struct MttTableState {
    pub table_id: GameId,
    pub hand_id: usize,
    pub btn: usize,
    pub sb: u64,
    pub bb: u64,
    pub ante: u64,
    pub players: Vec<MttTablePlayer>,
    pub next_game_start: u64,
}

impl MttTableState {
    pub fn new(table_id: GameId, sb: u64, bb: u64, ante: u64, players: Vec<MttTablePlayer>) -> Self {
        Self {
            table_id,
            hand_id: 0,
            btn: 0,
            sb,
            bb,
            ante,
            players,
            next_game_start: 0,
        }
    }

    /// If player already on table, returns false represents add failed.
    pub fn add_player(&mut self, player: &MttTablePlayer) -> bool {
        let exists = self.players.iter().any(|p| p.id == player.id);

        if exists {
            return false;
        } else {
            self.players.push(MttTablePlayer::new(
                player.id,
                player.chips,
                player.time_cards,
            ));

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
    pub fn new(
        player_id: u64,
        chips: u64,
        chips_change: Option<ChipsChange>,
        status: PlayerResultStatus,
    ) -> Self {
        Self {
            player_id,
            chips,
            chips_change,
            status,
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
        ante: u64,
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
