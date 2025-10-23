//! A standalone evaluator for Omaha poker hands.
//!
//! This evaluator correctly applies the Omaha rule of using exactly
//! two hole cards and three board cards to form the best 5-card hand.

use crate::holdem_evaluator::{PlayerHand, Category, compare_kinds, compare_hands};
use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};

// Helper to convert card kind to a numeric value for sorting and comparison.
fn kind_to_order(card: &str) -> u8 {
    let (_, kind) = card.split_at(1);
    match kind {
        "a" => 14, "k" => 13, "q" => 12, "j" => 11, "t" => 10,
        "9" => 9, "8" => 8, "7" => 7, "6" => 6, "5" => 5,
        "4" => 4, "3" => 3, "2" => 2, _ => 0,
    }
}

// Helper to get the numeric rank of a hand category.
fn get_category_order(category: Category) -> u8 {
    match category {
        Category::RoyalFlush => 9,
        Category::StraightFlush => 8,
        Category::FourOfAKind => 7,
        Category::FullHouse => 6,
        Category::Flush => 5,
        Category::Straight => 4,
        Category::ThreeOfAKind => 3,
        Category::TwoPairs => 2,
        Category::Pair => 1,
        Category::HighCard => 0,
    }
}

// Helper to construct the final PlayerHand struct.
fn build_hand(category: Category, picks_slice: &[&str]) -> PlayerHand {
    let picks = picks_slice.iter().map(|s| s.to_string()).collect::<Vec<String>>();
    let mut value = vec![get_category_order(category)];
    value.extend(picks_slice.iter().map(|c| kind_to_order(c)));
    PlayerHand { category, picks, value }
}

// Helper for kind-based hands (pairs, trips, etc.) to ensure correct card ordering for comparison.
fn build_hand_from_kinds(category: Category, sorted_kind_groups: &[(u8, u8)], all_cards: &[&str]) -> PlayerHand {
    let mut reordered_cards: Vec<&str> = Vec::with_capacity(5);
    let mut used_cards = HashSet::new();

    // Add cards based on the sorted kind groups (e.g., three of a kind first, then kickers)
    for (kind, _) in sorted_kind_groups {
        for card in all_cards {
            if kind_to_order(card) == *kind && !used_cards.contains(card) {
                reordered_cards.push(card);
                used_cards.insert(card);
            }
        }
    }

    build_hand(category, &reordered_cards[0..5])
}


/// Evaluates a 5-card hand and returns its classification.
fn evaluate_five_card_hand(cards: &[&str]) -> PlayerHand {
    let mut sorted_cards: Vec<&str> = cards.to_vec();
    sorted_cards.sort_by(|a, b| compare_kinds(a, b));

    // Check for flush
    let first_suit = sorted_cards[0].chars().next().unwrap();
    let is_flush = sorted_cards.iter().all(|c| c.starts_with(first_suit));

    // Check for straight
    let kinds: Vec<u8> = sorted_cards.iter().map(|c| kind_to_order(c)).collect();
    let is_wheel = kinds == [14, 5, 4, 3, 2]; // A-2-3-4-5
    let is_straight = is_wheel || kinds.windows(2).all(|w| w[0] == w[1] + 1);

    if is_straight && is_flush {
        if kinds[0] == 14 && kinds[1] == 13 { // Ace-high straight flush
            return build_hand(Category::RoyalFlush, &sorted_cards);
        }
        let hand_cards = if is_wheel {
            vec![sorted_cards[1], sorted_cards[2], sorted_cards[3], sorted_cards[4], sorted_cards[0]]
        } else {
            sorted_cards.clone()
        };
        return build_hand(Category::StraightFlush, &hand_cards);
    }

    // Analyze card kinds for pairs, etc.
    let mut kind_counts: HashMap<u8, u8> = HashMap::new();
    for kind in &kinds {
        *kind_counts.entry(*kind).or_insert(0) += 1;
    }

    let mut counts: Vec<(u8, u8)> = kind_counts.into_iter().collect();
    counts.sort_by(|(k1, c1), (k2, c2)| c2.cmp(c1).then_with(|| k2.cmp(k1)));
    let count_values: Vec<u8> = counts.iter().map(|(_, c)| *c).collect();

    if count_values[0] == 4 {
        return build_hand_from_kinds(Category::FourOfAKind, &counts, &sorted_cards);
    }
    if count_values == vec![3, 2] {
        return build_hand_from_kinds(Category::FullHouse, &counts, &sorted_cards);
    }
    if is_flush {
        return build_hand(Category::Flush, &sorted_cards);
    }
    if is_straight {
        let hand_cards = if is_wheel {
             vec![sorted_cards[1], sorted_cards[2], sorted_cards[3], sorted_cards[4], sorted_cards[0]]
        } else {
            sorted_cards.clone()
        };
        return build_hand(Category::Straight, &hand_cards);
    }
    if count_values[0] == 3 {
        return build_hand_from_kinds(Category::ThreeOfAKind, &counts, &sorted_cards);
    }
    if count_values == vec![2, 2, 1] {
        return build_hand_from_kinds(Category::TwoPairs, &counts, &sorted_cards);
    }
    if count_values[0] == 2 {
        return build_hand_from_kinds(Category::Pair, &counts, &sorted_cards);
    }

    build_hand(Category::HighCard, &sorted_cards)
}

// Helper to generate combinations, e.g., C(n, k)
fn combinations<T: Copy>(pool: &[T], k: usize) -> Vec<Vec<T>> {
    if k == 0 { return vec![vec![]]; }
    if pool.is_empty() { return vec![]; }
    let first = pool[0];
    let rest = &pool[1..];
    let mut result = combinations(rest, k - 1)
        .into_iter()
        .map(|mut sub| { sub.insert(0, first); sub })
        .collect::<Vec<_>>();
    result.extend(combinations(rest, k));
    result
}

/// Evaluates an Omaha hand by checking all possible combinations of 2 hole
/// cards and 3 board cards, returning the best possible 5-card hand.
pub fn evaluate_omaha_hand<'a>(hole_cards: &[&'a str], board: &[&'a str]) -> PlayerHand {
    let hole_combos = combinations(hole_cards, 2);
    let board_combos = combinations(board, 3);
    let mut best_hand: Option<PlayerHand> = None;

    for hc in &hole_combos {
        for bc in &board_combos {
            let mut current_cards = hc.clone();
            current_cards.extend_from_slice(bc);

            let hand = evaluate_five_card_hand(&current_cards);

            if let Some(ref best) = best_hand {
                if compare_hands(&hand.value, &best.value) == Ordering::Greater {
                    best_hand = Some(hand);
                }
            } else {
                best_hand = Some(hand);
            }
        }
    }
    // There will always be a best hand from the 60 combinations.
    best_hand.unwrap()
}
