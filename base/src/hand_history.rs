use borsh::{BorshDeserialize, BorshSerialize};

use crate::{essential::{GameEvent, Street}, evaluator::Category};
use std::collections::BTreeMap;

#[derive(Debug, BorshDeserialize, BorshSerialize, PartialEq)]
pub enum ChipsChange {
    NoUpdate,
    Add(u64),
    Sub(u64),
}

#[derive(Debug, BorshDeserialize, BorshSerialize, PartialEq)]
pub enum BlindType {
    Bb,
    Ante,
    Stradle,
}

#[derive(Debug, BorshDeserialize, BorshSerialize, PartialEq)]
pub struct BlindInfo {
    pub blind_type: BlindType,
    pub amount: u64,
}

#[derive(Debug, BorshDeserialize, BorshSerialize, PartialEq)]
pub struct PlayerAction {
    pub addr: String,
    pub event: GameEvent,
}

#[derive(Debug, Default, BorshDeserialize, BorshSerialize, PartialEq)]
pub struct StreetActions {
    pub street: Street,
    pub pot: u64,
    pub actions: Vec<PlayerAction>,
}

#[derive(Debug, BorshDeserialize, BorshSerialize, PartialEq)]
pub struct Showdown {
    pub hole_cards: Vec<String>,
    pub category: Category,
    pub picks: Vec<String>,
}

#[derive(Default, Debug, BorshDeserialize, BorshSerialize, PartialEq)]
pub struct HandHistory {
    pub board: Vec<String>,
    pub blinds: Vec<BlindInfo>,
    // A list of street logs
    pub street_actions: Vec<StreetActions>,
    // Player address -> showdown info
    pub showdowns: BTreeMap<String, Showdown>,
    // Player address -> chips change
    pub chips_change: BTreeMap<String, ChipsChange>,
}
