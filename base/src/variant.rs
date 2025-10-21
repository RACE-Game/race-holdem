use race_api::prelude::*;
use std::collections::{HashMap, BTreeMap};
use crate::essential::Pot;
use crate::hand_history::Showdown;

pub struct EvaluateHandsOutput {
    pub winner_sets: Vec<Vec<u64>>,
    pub showdown_map: BTreeMap<u64, Showdown>,
}

pub trait GameVariant: Default + BorshDeserialize + BorshSerialize {

    /// Returns the number of hole cards to deal to each player.
    fn hole_card_count(&self) -> usize;

    /// Evaluates all hands at showdown and returns ranked lists of winner IDs.
    fn evaluate_hands(
        &self,
        board: &[String],
        hand_index_map: &BTreeMap<u64, Vec<usize>>,
        revealed_cards: &HashMap<usize, String>,
    ) -> HandleResult<EvaluateHandsOutput>;

    /// Validates a raise amount (to handle No-Limit vs. Pot-Limit)
    fn validate_raise_amount(
        &self,
        player_chips: u64,
        betted: u64,
        raise_amount: u64,
        street_bet: u64,
        min_raise: u64,
        pots: &[Pot],
    ) -> HandleResult<()>;
}
