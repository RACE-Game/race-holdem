// Omaha implementation for GameVariant

use race_api::prelude::*;
use std::collections::{BTreeMap, HashMap};
use crate::hand_history::Showdown;
use crate::variant::{EvaluateHandsOutput, GameVariant};
use crate::holdem_evaluator::{PlayerHand, compare_hands};
use crate::omaha_evaluator;
use crate::errors;
use crate::essential::Pot;

#[derive(BorshSerialize, BorshDeserialize, Default, Debug, PartialEq, Clone)]
pub struct OmahaVariant;

impl GameVariant for OmahaVariant {
    fn hole_card_count(&self) -> usize {
        4
    }

    fn evaluate_hands(
        &self,
        board: &[String],
        hand_index_map: &BTreeMap<u64, Vec<usize>>,
        revealed_cards: &HashMap<usize, String>,
    ) -> HandleResult<EvaluateHandsOutput> {
        struct EvalResult {
            player_id: u64,
            hand: PlayerHand,
            showdown: Showdown,
        }

        let mut results: Vec<EvalResult> = Vec::with_capacity(hand_index_map.len());
        let board_cards: Vec<&str> = board.iter().map(String::as_str).collect();

        for (&player_id, indices) in hand_index_map.iter() {

            // Collect hole cards only if they exist in revealed_cards
            let mut hole_cards_str = Vec::new();
            let mut has_all_cards = true;

            for i in indices {
                if let Some(card) = revealed_cards.get(i) {
                    hole_cards_str.push(card.clone());
                } else {
                    // If any card is missing (player folded/left), mark as invalid
                    has_all_cards = false;
                    break;
                }
            }

            // Only proceed if we successfully retrieved exactly 4 cards
            if has_all_cards && hole_cards_str.len() == 4 {
                let hole_cards: Vec<&str> = hole_cards_str.iter().map(String::as_str).collect();

                // Use the new Omaha evaluator
                let hand = omaha_evaluator::evaluate_omaha_hand(&hole_cards, &board_cards);

                let showdown = Showdown {
                    hole_cards: hole_cards_str,
                    category: hand.category.clone(),
                    picks: hand.picks.iter().map(|x| x.to_string()).collect(),
                };
                results.push(EvalResult { player_id, hand, showdown });
            }
        }

        // Sort players from best hand to worst.
        results.sort_by(|a, b| compare_hands(&b.hand.value, &a.hand.value));

        // Group players into ranked sets (same as Hold'em implementation).
        let mut ranked_winners: Vec<Vec<u64>> = Vec::new();
        let mut showdown_map = BTreeMap::new();
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

    fn validate_bet_amount(
        &self,
        bet_amount: u64,
        bb: u64,
        player_chips: u64,
        pots: &[Pot],
    ) -> HandleResult<()> {
        // Check 1: The bet must not exceed the pot limit.
        let pot_before_action: u64 = pots.iter().map(|p| p.amount).sum::<u64>();
        if bet_amount > pot_before_action {
            return Err(errors::bet_exceeds_pot_limit());
        }

        // Check 2: The bet must meet the minimum bet requirement(1BB), unless it's an all-in.
        if bet_amount < bb && bet_amount != player_chips {
            return Err(errors::bet_amount_is_too_small());
        }

        Ok(())
    }

    fn validate_raise_amount(
        &self,
        raise_amount: u64,
        player_chips: u64,
        betted: u64,
        street_bet: u64,
        min_raise: u64,
        bet_sum_of_all_players: u64,
        pots: &[Pot],
    ) -> HandleResult<()> {
        // Check 1: The raise must not exceed the pot limit. This is the primary rule.
        let pot_before_action: u64 = pots.iter().map(|p| p.amount).sum::<u64>() + bet_sum_of_all_players;
        let call_amount = street_bet - betted;
        let max_raise = pot_before_action + call_amount;

        if raise_amount > max_raise {
            return Err(errors::raise_exceeds_pot_limit());
        }

        // Check 2: The raise must meet the minimum raise requirement, unless it's an all-in.
        let total_new_bet = betted + raise_amount;
        if total_new_bet < street_bet + min_raise && raise_amount != player_chips {
            return Err(errors::raise_amount_is_too_small());
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_raise_amount() {
        let v = OmahaVariant {};

        // bet 2, call 2 - bet 4, raise 10, raise ?
        let r = v.validate_raise_amount(
            1000,               // player_chips
            4,                  // betted
            12,                 // raise_amount
            10,                 // street_bet
            6,                  // min_raise
            14,                 // bet_sum_of_all_players
            &[Pot::new(vec![1, 2], 4)],
        );
        assert_eq!(r, Ok(()));

        // Pot limit-> 6(for call) + 18 (previous pot)
        // bet 2, call 2 - bet 4, raise 10, raise ?
        let r = v.validate_raise_amount(
            1000,               // player_chips
            4,                  // betted
            24,                 // raise_amount
            10,                 // street_bet
            6,                  // min_raise
            14,                 // bet_sum_of_all_players
            &[Pot::new(vec![1, 2], 4)],
        );

        assert_eq!(r, Ok(()));

        // Over pot limit
        // bet 2, call 2 - bet 4, raise 10, raise ?
        let r = v.validate_raise_amount(
            1000,               // player_chips
            4,                  // betted
            25,                 // raise_amount
            10,                 // street_bet
            6,                  // min_raise
            14,                 // bet_sum_of_all_players
            &[Pot::new(vec![1, 2], 4)],
        );

        assert_eq!(r, Err(errors::raise_exceeds_pot_limit()));
    }
}
