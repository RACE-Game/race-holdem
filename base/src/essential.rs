//! Holdem essentials such as bet, player, pot, street and so on.

use borsh::{BorshDeserialize, BorshSerialize};
use race_api::prelude::{CustomEvent, HandleError};
use std::collections::BTreeMap;

pub const ACTION_TIMEOUT_PREFLOP: u64 = 10_000;
pub const ACTION_TIMEOUT_POSTFLOP: u64 = 15_000;
pub const ACTION_TIMEOUT_TURN: u64 = 20_000;
pub const ACTION_TIMEOUT_RIVER: u64 = 30_000;
pub const ACTION_TIMEOUT_AFK: u64 = 1_000;
pub const MAX_ACTION_TIMEOUT_COUNT: u8 = 2;

pub const WAIT_TIMEOUT_DEFAULT: u64 = 5_000;
pub const WAIT_TIMEOUT_LAST_PLAYER: u64 = 5_000;
pub const WAIT_TIMEOUT_SHOWDOWN: u64 = 10_000;
pub const WAIT_TIMEOUT_RUNNER: u64 = 13_000;

pub const TIME_CARD_EXTRA_SECS: u64 = 30;
pub const DEFAULT_TIME_CARDS: u8 = 5;

/// Holdem Modes in which a specific table type is defined
#[derive(BorshSerialize, BorshDeserialize, Default, PartialEq, Debug, Clone, Copy)]
pub enum GameMode {
    #[default]
    Cash,
    Sng,
    Mtt,
}

/// Players' status during the entire game life
#[derive(BorshSerialize, BorshDeserialize, Default, PartialEq, Eq, Debug, Clone, Copy)]
pub enum PlayerStatus {
    #[default]
    Wait,       // the player is waiting for others' action
    Waitbb,     // the player (newly joined) is waiting to become the big blind
    Acted,      // the player has acted on current street
    Acting,     // the player is acting
    Allin,      // the player goes allin
    Fold,       // the player has folded
    Init,       // the player just joined the game, waiting for next hand to start
    Leave,      // the player just sent the Leave event
    Out,        // the player is sitted out
    Eliminated, // the player has zero chips
}

#[derive(BorshSerialize, BorshDeserialize, Debug, PartialEq)]
pub struct InternalPlayerJoin {
    pub id: u64,
    pub chips: u64,
}

/// Representation of a specific player in the game
#[derive(Default, BorshSerialize, BorshDeserialize, Debug, Clone, PartialEq, Eq)]
pub struct Player {
    pub id: u64,
    pub chips: u64,
    pub position: u8,         // Zero indexed
    pub status: PlayerStatus,
    pub timeout: u8,          // Count the times of action timeout.
    pub deposit: u64,         // The deposited amount.
    pub time_cards: u8,       // The number of time cards, each gives extra TIME_CARD_EXTRA_SECS for action.
    pub is_afk: bool,         // Is player away from keyboard.
}

impl Player {
    pub fn new(id: u64, chips: u64, position: u8, status: PlayerStatus, timeout: u8, deposit: u64, time_cards: u8) -> Player {
        Self {
            id, chips, position: position, status, timeout, deposit, time_cards, is_afk: false,
        }
    }

    pub fn new_with_timeout(id: u64, chips: u64, position: u8, timeout: u8) -> Player {
        Self {
            id,
            chips,
            position,
            status: PlayerStatus::default(),
            timeout,
            deposit: 0,
            time_cards: DEFAULT_TIME_CARDS,
            is_afk: false,
        }
    }

    /// Give timeout, deposit and time_cards a default value.
    /// For testing.
    pub fn new_with_defaults(
        id: u64,
        chips: u64,
        position: u8,
        status: PlayerStatus,
    ) -> Player {
        Self {
            id,
            chips,
            position,
            status,
            timeout: 0,
            deposit: 0,
            time_cards: DEFAULT_TIME_CARDS,
            is_afk: false,
        }
    }

    // New players joined game all init to the status `Waitbb`
    pub fn init(id: u64, chips: u64, position: u8, status: PlayerStatus) -> Player {
        Self {
            id,
            chips,
            position,
            status,
            timeout: 0,
            deposit: 0,
            time_cards: DEFAULT_TIME_CARDS,
            is_afk: false,
        }
    }

    pub fn next_to_act(&self) -> bool {
        match self.status {
            PlayerStatus::Allin
            | PlayerStatus::Fold
            | PlayerStatus::Init
            | PlayerStatus::Waitbb
            | PlayerStatus::Leave
            | PlayerStatus::Out => false,
            _ => true,
        }
    }

    // This fn indicates
    // 1. whether player goes all in
    // 2. the actual bet amount
    // 3. the player's remaining chips after the bet
    pub fn take_bet(&mut self, bet: u64) -> (bool, u64) {
        if bet < self.chips {
            println!("{} bets: {}", self.id, bet);
            self.chips -= bet;
            (false, bet)
        } else {
            let real_bet = self.chips;
            println!("{} ALL IN: {}", self.id, real_bet);
            self.chips = 0;
            (true, real_bet)
        }
    }
}

/// Representation of the player who should be acting at the moment
#[derive(BorshSerialize, BorshDeserialize, PartialEq, Debug, Clone)]
pub struct ActingPlayer {
    pub id: u64,
    pub position: u8,
    pub action_start: u64,
    pub clock: u64, // action clock
    pub time_card_clock: Option<u64>, // Some when a time card is in use
}

impl ActingPlayer {
    pub fn new(id: u64, position: u8, action_start: u64, clock: u64) -> ActingPlayer {
        Self { id, position, action_start, clock, time_card_clock: None }
    }

    pub fn new_with_time_card(id: u64, position: u8, action_start: u64, clock: u64) -> ActingPlayer {
        Self { id, position, action_start, clock, time_card_clock: Some(clock + TIME_CARD_EXTRA_SECS * 1000) }
    }
}

/// Representation of Holdem pot
#[derive(BorshSerialize, BorshDeserialize, PartialEq, Debug, Clone, Default)]
pub struct Pot {
    pub owners: Vec<u64>,
    pub winners: Vec<u64>,
    pub amount: u64,
}

impl Pot {
    pub fn new(owners: Vec<u64>, amount: u64) -> Self {
        Self {
            owners,
            winners: Vec::new(),
            amount,
        }
    }

    pub fn merge(&mut self, other: &Pot) -> Result<(), HandleError> {
        self.amount += other.amount;
        Ok(())
    }
}

/// Holdem Streets
#[derive(BorshSerialize, BorshDeserialize, PartialEq, Debug, Default, Copy, Clone)]
pub enum Street {
    #[default]
    Init,
    Preflop,
    Flop,
    Turn,
    River,
    Showdown,
}

/// Holdem stages for micro-management of the game
#[derive(Default, BorshSerialize, BorshDeserialize, PartialEq, Debug, Clone)]
pub enum HoldemStage {
    #[default]
    Init,
    ShareKey,
    Play,
    Runner,
    Settle,
    Showdown,
}

/// Holdem specific custom events.  Transactor will pass through such events to the game handler.
/// Also see [`race_api::event::Event::Custom`].
#[derive(BorshSerialize, BorshDeserialize, Debug, PartialEq, Clone)]
pub enum GameEvent {
    Bet(u64),
    Check,
    Call,
    Fold,
    Raise(u64),
    SitIn,       // One condition is back from AFK status
    SitOut,
    UseTimeCard, // Activate a time card, the time card will only be consumed when the extra time is used.
}

impl CustomEvent for GameEvent {}

/// The following structs are used for the front-end to display animations.
// A pot used for awarding winners
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, PartialEq)]
pub struct AwardPot {
    pub winners: Vec<u64>,
    pub amount: u64,
}

#[derive(BorshSerialize, BorshDeserialize, Debug, PartialEq, Clone)]
pub struct PlayerResult {
    pub id: u64,
    pub chips: u64,
    pub status: PlayerStatus,
    pub position: u8,
}

#[derive(BorshSerialize, BorshDeserialize, Debug, PartialEq, Clone)]
pub enum Display {
    DealCards,
    DealBoard {
        prev: usize,
        board: Vec<String>,
    },
    CollectBets {
        old_pots: Vec<Pot>,
        bet_map: BTreeMap<u64, u64>,
    },
    GameResult {
        award_pots: Vec<AwardPot>,
        player_map: BTreeMap<u64, PlayerResult>,
    },
}
