// Omaha implementation for GameVariant

use crate::variant::{EvaluateHandsOutput, GameVariant};
use crate::evaluator::evaluate_cards;
use race_api::prelude::*;

pub struct OmahaVariant {

}

impl GameVariant for HoldemVariant {
    fn hole_card_count(&self) -> usize {
        4
    }

    fn evaluate_hands(
        &self,
        board: &[&str],
        hole_cards: &[&str],
    ) -> HandleResult<EvaluateHandsOutput> {
        // Holdem doesn't distinguish hole cards and community cards

    }

    fn validate_raise_amount(
        &self,
        player_chips: u64,
        betted: u64,
        raise_amount: u64,
        street_bet: u64,
        min_raise: u64,
        pots: &[Pot],
    ) -> HandleResult<()> {

    }
}
