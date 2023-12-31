//! Holdem essentials such as bet, player, pot, street and so on.

use borsh::{BorshDeserialize, BorshSerialize};
use race_api::prelude::{CustomEvent, HandleError};
use std::collections::BTreeMap;

pub const MAX_ACTION_TIMEOUT_COUNT: u8 = 2;
pub const ACTION_TIMEOUT_PREFLOP: u64 = 12_000;
pub const ACTION_TIMEOUT_POSTFLOP: u64 = 15_000;
pub const ACTION_TIMEOUT_TURN: u64 = 20_000;
pub const ACTION_TIMEOUT_RIVER: u64 = 30_000;

pub const WAIT_TIMEOUT_DEFAULT: u64 = 5_000;
pub const WAIT_TIMEOUT_LAST_PLAYER: u64 = 5_000;
pub const WAIT_TIMEOUT_SHOWDOWN: u64 = 7_000;
pub const WAIT_TIMEOUT_RUNNER: u64 = 13_000;

pub const RAKE_SLOT_ID: u8 = 0;

#[derive(BorshSerialize, BorshDeserialize, Default, PartialEq, Debug, Clone, Copy)]
pub enum GameMode {
    #[default]
    Cash,
    Sng,
    Mtt,
}

#[derive(BorshSerialize, BorshDeserialize, Default, PartialEq, Debug, Clone, Copy)]
pub enum PlayerStatus {
    #[default]
    Wait,
    Acted,
    Acting,
    Allin,
    Fold,
    Init, // Indicating new players ready for the next hand
    Leave,
    Out,
}

#[derive(BorshSerialize, BorshDeserialize, Debug, PartialEq)]
pub struct InternalPlayerJoin {
    pub addr: String,
    pub chips: u64,
}

#[derive(BorshSerialize, BorshDeserialize, Debug)]
pub struct Player {
    pub addr: String,
    pub chips: u64,
    pub position: usize, // zero indexed
    pub status: PlayerStatus,
    pub timeout: u8, // count the times of action timeout
}

impl PartialEq for Player {
    fn eq(&self, other: &Self) -> bool {
        self.addr == other.addr && self.position == other.position
    }
}

impl Player {
    pub fn new(addr: String, chips: u64, position: u16, timeout: u8) -> Player {
        Self {
            addr,
            chips,
            position: position as usize,
            status: PlayerStatus::default(),
            timeout,
        }
    }

    pub fn new_with_status(
        addr: String,
        chips: u64,
        position: usize,
        status: PlayerStatus,
    ) -> Player {
        Self {
            addr,
            chips,
            position,
            status,
            timeout: 0,
        }
    }

    pub fn init(addr: String, chips: u64, position: u16) -> Player {
        Self {
            addr,
            chips,
            position: position as usize,
            status: PlayerStatus::Init,
            timeout: 0,
        }
    }

    pub fn addr(&self) -> String {
        self.addr.clone()
    }

    pub fn next_to_act(&self) -> bool {
        match self.status {
            PlayerStatus::Allin | PlayerStatus::Fold | PlayerStatus::Init | PlayerStatus::Leave | PlayerStatus::Out => false,
            _ => true,
        }
    }

    // This fn indicates
    // 1. whether player goes all in
    // 2. the actual bet amount
    // 3. the player's remaining chips after the bet
    pub fn take_bet(&mut self, bet: u64) -> (bool, u64) {
        if bet < self.chips {
            println!("{} bets: {}", &self.addr, bet);
            self.chips -= bet;
            (false, bet)
        } else {
            let real_bet = self.chips;
            println!("{} ALL IN: {}", &self.addr, real_bet);
            self.chips = 0;
            (true, real_bet)
        }
    }
}

#[derive(BorshSerialize, BorshDeserialize, PartialEq, Debug, Clone)]
pub struct ActingPlayer {
    pub addr: String,
    pub position: usize,
    pub clock: u64, // action clock
}

#[derive(BorshSerialize, BorshDeserialize, PartialEq, Debug, Clone)]
pub struct Pot {
    pub owners: Vec<String>,
    pub winners: Vec<String>,
    pub amount: u64,
}

impl Pot {
    pub fn new() -> Self {
        Self {
            owners: Vec::<String>::new(),
            winners: Vec::<String>::new(),
            amount: 0,
        }
    }

    pub fn merge(&mut self, other: &Pot) -> Result<(), HandleError> {
        self.amount += other.amount;
        Ok(())
    }
}

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

#[derive(Default, BorshSerialize, BorshDeserialize, PartialEq, Debug)]
pub enum HoldemStage {
    #[default]
    Init,
    ShareKey,
    Play,
    Runner,
    Settle,
    Showdown,
}

#[derive(BorshSerialize, BorshDeserialize, PartialEq, Debug)]
pub struct HoldemAccount {
    pub sb: u64,
    pub bb: u64,
    pub ante: u64,
    pub rake: u16, // an integer representing the rake (per thousand)
    pub theme: Option<String>, // an optional theme identifier
}

impl Default for HoldemAccount {
    fn default() -> Self {
        Self {
            sb: 10,
            bb: 20,
            ante: 0,
            rake: 3u16,
            theme: None
        }
    }
}

#[derive(BorshSerialize, BorshDeserialize, Debug, PartialEq)]
pub enum GameEvent {
    Bet(u64),
    Check,
    Call,
    Fold,
    Raise(u64),
}

impl CustomEvent for GameEvent {}

// A pot used for awarding winners
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, PartialEq)]
pub struct AwardPot {
    pub winners: Vec<String>,
    pub amount: u64,
}

#[derive(BorshSerialize, BorshDeserialize, Debug, PartialEq)]
pub struct PlayerResult {
    pub addr: String,
    pub chips: u64,
    pub prize: Option<u64>,
    pub status: PlayerStatus,
    pub position: usize,
}

/// Used for animation (with necessary audio effects)
#[derive(BorshSerialize, BorshDeserialize, Debug, PartialEq)]
pub enum Display {
    DealCards,
    DealBoard {
        prev: usize,
        board: Vec<String>,
    },
    CollectBets {
        bet_map: BTreeMap<String, u64>,
    },
    AwardPots {
        pots: Vec<AwardPot>,
    },
    GameResult {
        player_map: BTreeMap<String, PlayerResult>,
    },
}
