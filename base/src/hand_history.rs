use borsh::{BorshDeserialize, BorshSerialize};
use race_api::prelude::HandleError;

use crate::{
    errors,
    essential::Street,
    evaluator::Category,
};
use std::collections::BTreeMap;

#[derive(Debug, BorshDeserialize, BorshSerialize, PartialEq, Clone)]
pub enum ChipsChange {
    NoUpdate,
    Add(u64),
    Sub(u64),
}

#[derive(Debug, BorshDeserialize, BorshSerialize, PartialEq, Clone)]
pub enum BlindType {
    Sb,
    Bb,
    Ante,
    Stradle,
}

#[derive(Debug, BorshDeserialize, BorshSerialize, PartialEq, Clone)]
pub struct BlindBet {
    pub id: u64,
    pub blind_type: BlindType,
    pub amount: u64,
    pub is_allin: bool,
}

impl BlindBet {
    pub fn new(id: u64, blind_type: BlindType, amount: u64, is_allin: bool) -> Self {
        Self {
            id,
            blind_type,
            amount,
            is_allin,
        }
    }
}

#[derive(Debug, BorshDeserialize, BorshSerialize, PartialEq, Clone)]
pub struct PlayerAction {
    id: u64,
    event: PlayerActionEvent,
}

impl PlayerAction {
    pub fn new_bet(id: u64, real_bet_amount: u64, is_allin: bool) -> Self {
        Self { id, event: PlayerActionEvent::Bet { amount: real_bet_amount, is_allin } }
    }
    pub fn new_raise(id: u64, real_raise_to_amount: u64, is_allin: bool) -> Self {
        Self { id, event: PlayerActionEvent::Raise { amount: real_raise_to_amount, is_allin } }
    }
    pub fn new_call(id: u64, real_call_amount: u64, is_allin: bool) -> Self {
        Self { id, event: PlayerActionEvent::Call { amount: real_call_amount, is_allin } }
    }
    pub fn new_check(id: u64) -> Self {
        Self { id, event: PlayerActionEvent::Check }
    }
    pub fn new_fold(id: u64) -> Self {
        Self { id, event: PlayerActionEvent::Fold }
    }
}

#[derive(Debug, BorshDeserialize, BorshSerialize, PartialEq, Clone)]
pub enum PlayerActionEvent {
    Bet {
        amount: u64,
        is_allin: bool,
    },
    Check,
    Call{
        amount: u64,
        is_allin: bool,
    },
    Fold,
    Raise{
        amount: u64,
        is_allin: bool,
    },
}

#[derive(Debug, Default, BorshDeserialize, BorshSerialize, PartialEq, Clone)]
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

#[derive(Default, Debug, BorshDeserialize, BorshSerialize, PartialEq, Clone)]
pub struct HandHistory {
    pub valid: bool,
    pub board: Vec<String>,
    pub blinds: Vec<BlindBet>,
    // actions for each street
    pub preflop: StreetActions,
    pub flop: StreetActions,
    pub turn: StreetActions,
    pub river: StreetActions,
    // Player address -> showdown info
    pub showdowns: BTreeMap<u64, Showdown>,
    // Player address -> chips change
    pub chips_change: BTreeMap<u64, ChipsChange>,
}

impl HandHistory {
    pub fn set_board(&mut self, board: Vec<String>) {
        self.board = board;
    }

    pub fn set_chips_change(&mut self, chips_change_map: &BTreeMap<u64, i64>) {
        for (id, change) in chips_change_map.iter() {
            let change = *change;
            if change > 0 {
                self.chips_change
                    .insert(*id, ChipsChange::Add(change as u64));
            } else if change < 0 {
                self.chips_change
                    .insert(*id, ChipsChange::Sub((-change) as u64));
            }
        }
    }

    pub fn set_blinds_infos(&mut self, blinds: Vec<BlindBet>) {
        self.blinds = blinds
    }

    pub fn add_blinds_info(&mut self, blind: BlindBet) {
        self.blinds.push(blind)
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

    pub fn add_showdown(&mut self, id: u64, showdown: Showdown) {
        self.showdowns.insert(id, showdown);
    }
}
