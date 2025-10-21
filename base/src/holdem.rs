// Holdem implementation for GameVariant

use crate::hand_history::Showdown;
use crate::variant::{EvaluateHandsOutput, GameVariant};
use crate::evaluator::evaluate_cards;
use race_api::prelude::*;

#[derive(BorshSerialize, BorshDeserialize, Default, Debug, PartialEq, Clone)]
pub struct HoldemVariant {

}

impl GameVariant for HoldemVariant {
    fn hole_card_count(&self) -> usize {
        2
    }

    fn evaluate_hands(
        &self,
        board: &[&str],
        hole_cards: &[&str],
    ) -> HandleResult<EvaluateHandsOutput> {
        // A temporary struct to hold all evaluation results before sorting
        struct EvalResult {
            player_id: u64,
            hand: PlayerHand,
            showdown: Showdown,
        }

        let mut results: Vec<EvalResult> = Vec::with_capacity(hand_index_map.len());
        let mut showdown_map = BTreeMap::new();

        // Step 1: Evaluate each player's hand and build the Showdown struct.
        for (&player_id, indices) in hand_index_map.iter() {
            let card1 = revealed.get(&indices[0]).ok_or_else(errors::first_hole_card_error)?;
            let card2 = revealed.get(&indices[1]).ok_or_else(errors::second_hole_card_error)?;
            let hole_cards = vec![card1.clone(), card2.clone()];

            let all_cards = create_cards(board, &[card1.as_str(), card2.as_str()]);

            // The evaluator now returns a richer PlayerHand struct
            let hand = evaluate_cards(all_cards);

            let showdown = Showdown {
                hole_cards,
                category: hand.category.clone(),
                picks: hand.picks.clone(),
            };
            results.push(EvalResult { player_id, hand, showdown });
        }

        // Step 2: Sort players from best hand to worst.
        results.sort_by(|a, b| compare_hands(&b.hand.value, &a.hand.value));

        // Step 3: Group players into ranked sets and build the final data structures.
        let mut ranked_winners: Vec<Vec<u64>> = Vec::new();
        let mut current_value = None;

        for result in results.into_iter() {
            if Some(&result.hand.value) != current_value.as_ref() {
                current_value = Some(result.hand.value.clone());
                ranked_winners.push(Vec::new());
            }

            if let Some(last_tier) = ranked_winners.last_mut() {
                last_tier.push(result.player_id);
            }

            showdown_map.insert(result.player_id, result.showdown);
        }

        Ok(EvaluateHandsOutput {
            winner_sets: ranked_winners,
            showdown_map,
        })
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
        let total_new_bet = player_bet + raise_amount;

        // An all-in is always a valid raise amount, even if it's less than a min-raise.
        if raise_amount == player_chips {
            return Ok(());
        }

        // Otherwise, the new total bet must be at least the current bet plus the minimum raise.
        if total_new_bet < street_bet + min_raise {
            return Err(errors::raise_amount_is_too_small());
        }

        Ok(())
    }
}
