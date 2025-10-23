use borsh::{BorshDeserialize, BorshSerialize};
use race_api::prelude::*;

pub const DEFAULT_TIME_CARDS: u8 = 5;

#[derive(Debug, Clone, BorshSerialize, BorshDeserialize, Default, PartialEq, Eq)]
pub struct MttTablePlayer {
    pub id: u64,
    pub chips: u64,
    pub position: u8,
    pub time_cards: u8,
}

impl MttTablePlayer {
    pub fn new(id: u64, chips: u64, position: u8, time_cards: u8) -> Self {
        Self {
            id,
            chips,
            position,
            time_cards,
        }
    }

    /// Give time_cards a default value.
    /// For testing
    pub fn new_with_defaults(id: u64, chips: u64, position: u8) -> Self {
        Self::new(id, chips, position, DEFAULT_TIME_CARDS)
    }
}

#[derive(Debug, Clone, BorshSerialize, BorshDeserialize, Default, PartialEq, Eq)]
pub struct MttTableSitin {
    pub id: u64,
    pub chips: u64,
    // How many time cards the player still has
    pub time_cards: u8,
    // Whether it's the first time for this player to sit on a table(include first entry and rebuy),
    // which implies the player is not moved from another table.
    // For players have this flag, they must wait a BB to get dealt.
    pub first_time_sit: bool,
}

impl MttTableSitin {
    pub fn new(id: u64, chips: u64, time_cards: u8, first_time_sit: bool) -> Self {
        Self {
            id,
            chips,
            time_cards,
            first_time_sit,
        }
    }

    /// Give time_cards a default value.
    /// For testing
    pub fn new_with_defaults(id: u64, chips: u64) -> Self {
        Self::new(id, chips, DEFAULT_TIME_CARDS, true)
    }
}


#[derive(Debug, Clone, BorshSerialize, BorshDeserialize, Default, PartialEq, Eq)]
pub struct MttTablePlayerInit {
    pub id: u64,
    pub chips: u64,
}

impl MttTablePlayerInit {
    pub fn new(id: u64, chips: u64) -> Self {
        Self { id, chips }
    }
}

#[derive(Default, Debug, Clone, BorshSerialize, BorshDeserialize, PartialEq, Eq)]
pub struct MttTableInit {
    pub table_id: usize,
    pub sb: u64,
    pub bb: u64,
    pub ante: u64,
    pub players: Vec<MttTablePlayerInit>,
}

impl MttTableInit {
    pub fn new(
        table_id: usize,
        sb: u64,
        bb: u64,
        ante: u64,
        players: Vec<MttTablePlayerInit>,
    ) -> Self {
        Self {
            table_id,
            sb,
            bb,
            ante,
            players,
        }
    }
}

impl From<MttTableInit> for MttTableState {
    fn from(init: MttTableInit) -> Self {

        let players = init.players.iter().enumerate().map(|(idx, p)|
            MttTablePlayer::new(p.id, p.chips, idx as u8, DEFAULT_TIME_CARDS)
        ).collect();

        Self {
            table_id: init.table_id,
            hand_id: 0,
            btn: 0,
            sb: init.sb,
            bb: init.bb,
            ante: init.ante,
            players,
            next_game_start: 0,
        }
    }
}

#[derive(Default, Debug, Clone, BorshSerialize, BorshDeserialize, PartialEq, Eq)]
pub struct MttTableState {
    pub table_id: usize,
    pub hand_id: usize,
    pub btn: u8,
    pub sb: u64,
    pub bb: u64,
    pub ante: u64,
    pub players: Vec<MttTablePlayer>,
    pub next_game_start: u64,
}

impl MttTableState {
    pub fn new(
        table_id: usize,
        sb: u64,
        bb: u64,
        ante: u64,
        players: Vec<MttTablePlayer>,
    ) -> Self {
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

    /// Remove player on table, always return true as its an idempotent function.
    pub fn remove_player(&mut self, player_id: u64) -> bool {
        self.players.retain(|p| p.id != player_id);
        return true;
    }

    pub fn find_position(&self, table_size: u8) -> Option<u8> {
        for i in 0..table_size {
            if self
                .players
                .iter()
                .find(|p| p.position == i)
                .is_none()
            {
                return Some(i);
            }
        }
        return None;
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
    pub position: u8, // The player's position in this hand
    pub status: PlayerResultStatus,
}

impl PlayerResult {
    pub fn new(
        player_id: u64,
        chips: u64,
        chips_change: Option<ChipsChange>,
        position: u8,
        status: PlayerResultStatus,
    ) -> Self {
        Self {
            player_id,
            chips,
            chips_change,
            position,
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
        table_id: usize,
        btn: u8,
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


/// Get a relative position(to BTN) for given player id.
/// BTN is 0, SB is 1, BB is 2, and so on.
pub fn get_relative_position(player_id: u64, btn: u8, player_results: &[PlayerResult]) -> Option<u8> {
    let player_position = player_results.iter().find(|p| p.player_id == player_id)?.position;
    let mut positions: Vec<u8> = player_results.iter().map(|p| p.position).collect();
    let mid = positions.iter().position(|p| *p == btn)?;
    positions.rotate_left(mid);
    positions.iter().position(|p| *p == player_position).map(|i| i as u8)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_relative_position() {
        let player_results = vec![
            PlayerResult::new(1, 1000, None, 1, PlayerResultStatus::Normal),
            PlayerResult::new(2, 1000, None, 2, PlayerResultStatus::Normal),
            PlayerResult::new(3, 1000, None, 3, PlayerResultStatus::Normal),
            PlayerResult::new(4, 1000, None, 4, PlayerResultStatus::Normal),
            PlayerResult::new(5, 1000, None, 5, PlayerResultStatus::Normal),
            PlayerResult::new(6, 1000, None, 6, PlayerResultStatus::Normal),
        ];

        let btn = 4;
        assert_eq!(Some(0), get_relative_position(4, btn, &player_results));
        assert_eq!(Some(3), get_relative_position(1, btn, &player_results));
        assert_eq!(Some(1), get_relative_position(5, btn, &player_results));
    }
}
