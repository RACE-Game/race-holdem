use borsh::{BorshDeserialize, BorshSerialize};
use race_api::prelude::HandleError;

use crate::{
    errors,
    essential::{GameEvent, Street},
    evaluator::Category,
};
use std::collections::BTreeMap;

#[derive(Debug, BorshDeserialize, BorshSerialize, PartialEq)]
pub enum ChipsChange {
    NoUpdate,
    Add(u64),
    Sub(u64),
}

#[derive(Debug, BorshDeserialize, BorshSerialize, PartialEq)]
pub enum BlindType {
    Sb,
    Bb,
    Ante,
    Stradle,
}

#[derive(Debug, BorshDeserialize, BorshSerialize, PartialEq)]
pub struct BlindInfo {
    pub addr: String,
    pub blind_type: BlindType,
    pub amount: u64,
}

impl BlindInfo {
    pub fn new<S: Into<String>>(addr: S, blind_type: BlindType, amount: u64) -> Self {
        Self {
            addr: addr.into(),
            blind_type,
            amount,
        }
    }
}

#[derive(Debug, BorshDeserialize, BorshSerialize, PartialEq)]
pub struct PlayerAction {
    pub addr: String,
    pub event: GameEvent,
}

impl PlayerAction {
    pub fn new<S: Into<String>>(addr: S, event: GameEvent) -> Self {
        Self {
            addr: addr.into(),
            event,
        }
    }
}

#[derive(Debug, Default, BorshDeserialize, BorshSerialize, PartialEq)]
pub struct StreetActions {
    pub pot: u64,
    pub actions: Vec<PlayerAction>,
}

#[derive(Debug, BorshDeserialize, BorshSerialize, PartialEq, Clone)]
pub struct Showdown {
    pub hole_cards: Vec<String>,
    pub category: Category,
    pub picks: Vec<String>,
}

#[derive(Default, Debug, BorshDeserialize, BorshSerialize, PartialEq)]
pub struct HandHistory {
    pub board: Vec<String>,
    pub blinds: Vec<BlindInfo>,
    // actions for each street
    pub preflop: StreetActions,
    pub flop: StreetActions,
    pub turn: StreetActions,
    pub river: StreetActions,
    // Player address -> showdown info
    pub showdowns: BTreeMap<String, Showdown>,
    // Player address -> chips change
    pub chips_change: BTreeMap<String, ChipsChange>,
}

impl HandHistory {
    pub fn set_board(&mut self, board: Vec<String>) {
        self.board = board;
    }

    pub fn set_chips_change(&mut self, chips_change_map: &BTreeMap<String, i64>) {
        for (addr, change) in chips_change_map.iter() {
            let change = *change;
            if change > 0 {
                self.chips_change
                    .insert(addr.to_owned(), ChipsChange::Add(change as u64));
            } else if change < 0 {
                self.chips_change
                    .insert(addr.to_owned(), ChipsChange::Sub((-change) as u64));
            }
        }
    }

    pub fn set_blinds_infos(&mut self, blinds: Vec<BlindInfo>) {
        self.blinds = blinds
    }

    pub fn set_pot(&mut self, street: Street, pot: u64) {
        match street {
            Street::Init | Street::Showdown => (),
            Street::Preflop => {
                self.preflop.pot = pot;
            }
            Street::Flop => {
                self.flop.pot = pot;
            }
            Street::Turn => {
                self.turn.pot = pot;
            }
            Street::River => {
                self.river.pot = pot;
            }
        }
    }

    pub fn add_action(&mut self, street: Street, action: PlayerAction) -> Result<(), HandleError> {
        match street {
            Street::Init | Street::Showdown => {
                return Err(errors::internal_unexpected_street());
            }
            Street::Preflop => {
                self.preflop.actions.push(action);
            }
            Street::Flop => {
                self.flop.actions.push(action);
            }
            Street::Turn => {
                self.turn.actions.push(action);
            }
            Street::River => {
                self.river.actions.push(action);
            }
        };

        Ok(())
    }

    pub fn add_showdown<S: Into<String>>(&mut self, addr: S, showdown: Showdown) {
        self.showdowns.insert(addr.into(), showdown);
    }
}
