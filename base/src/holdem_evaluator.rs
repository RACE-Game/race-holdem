//! Functions and structs used to compare (evaluate) players' hands

use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};

use borsh::{BorshDeserialize, BorshSerialize};

/// Cards are consisted of 5 community cards + 2 hole cards.
/// Each card is represented with a string literal where
/// suit comes first, then kind. For example: "ca" represents Club Ace.
/// A hand (or picks) is the best 5 out of 7.
/// Cards can be sorted in two ways:
/// 1. by their kinds, for finding straights;
/// 2. by grouped kinds, for pairs, full house, or three/four of a kind.
pub fn create_cards<'a>(community_cards: &[&'a str], hole_cards: &[&'a str]) -> Vec<&'a str> {
    let mut cards: Vec<&str> = Vec::with_capacity(7);
    cards.extend_from_slice(community_cards);
    cards.extend_from_slice(hole_cards);
    cards
}

fn kind_to_order(card: &str) -> u8 {
    let (_, kind) = card.split_at(1);
    match kind {
        "a" => 14,
        "k" => 13,
        "q" => 12,
        "j" => 11,
        "t" => 10,
        "9" => 9,
        "8" => 8,
        "7" => 7,
        "6" => 6,
        "5" => 5,
        "4" => 4,
        "3" => 3,
        "2" => 2,
        _ => 0,
    }
}

/// After sorting, higher card (kind) will come first in the vec.
/// Input:  "ck" "ha"
/// Output: Ordering::Less
pub fn compare_kinds(card1: &str, card2: &str) -> Ordering {
    let order1 = kind_to_order(card1);
    let order2 = kind_to_order(card2);

    if order2 > order1 {
        Ordering::Greater
    } else if order2 < order1 {
        Ordering::Less
    } else {
        Ordering::Equal
    }
}

pub fn validate_cards(cards: &Vec<&str>) -> bool {
    if cards.len() == 7 {
        true
    } else {
        false
    }
}

fn find_kicker<'a>(cards: &[&'a str]) -> &'a str {
    let mut best_card = cards[0];
    for &card in cards.iter().skip(1) {
        if compare_kinds(card, best_card) == Ordering::Less {
            best_card = card;
        }
    }
    best_card
}

/// Sort the 7 cards by the number of suited kinds.
/// If two groups have equal number of cards, the higher-kind suit wins:
/// Input:  ["ht", "s8", "st", "c8", "h5", "d3", "h3"]
/// Output: ["ht", "st", "s8", "c8", "h5", "h3", "d3"]
fn sort_suited_cards<'a>(cards: &Vec<&'a str>) -> Vec<&'a str> {
    // Group cards by their kinds
    let cards_to_kinds: Vec<u8> = cards.iter().map(|&c| kind_to_order(c)).collect();
    let mut groups: HashMap<u8, Vec<&str>> = HashMap::with_capacity(7);
    for (idx, kind) in cards_to_kinds.into_iter().enumerate() {
        groups
            .entry(kind)
            .and_modify(|grp| grp.push(cards[idx]))
            .or_insert(vec![cards[idx]]);
    }
    // Create a vec of key-value to sort
    let mut to_sort: Vec<(u8, Vec<&str>)> = groups.into_iter().collect();

    // Sort the (kind, cards) in the vec
    to_sort.sort_by(|(k1, c1), (k2, c2)| -> Ordering {
        if c2.len() > c1.len() {
            Ordering::Greater
        } else if c2.len() == c1.len() {
            if k2 > k1 {
                Ordering::Greater
            } else {
                Ordering::Less
            }
        } else {
            Ordering::Less
        }
    });

    let result: Vec<&str> = to_sort
        .into_iter()
        .fold(Vec::with_capacity(7), |mut acc, (_, cs)| {
            acc.extend_from_slice(&cs);
            acc
        });
    result
}

// ============================================================
// Most fns below will assume that `cards' have been sorted
// using the fns above, in the order of from high to low
// ============================================================

/// Used to detect the type of SameKinds: One Pair, Two Pairs, FullHouse, etc.
#[derive(Debug, PartialEq, BorshSerialize, BorshDeserialize, Copy, Clone)]
pub enum Category {
    RoyalFlush,
    StraightFlush,
    FourOfAKind,
    FullHouse,
    Flush,
    Straight,
    ThreeOfAKind,
    TwoPairs,
    Pair,
    HighCard,
}

#[derive(Debug)]
pub struct PlayerHand {
    pub category: Category,  // rankings
    pub picks: Vec<String>,  // Best 5 out of 7
    pub value: Vec<u8>,      // [value, category_order ...]
}

/// Given the vec of kind orders, tag the category order value in the first place
fn tag_value(picked: &Vec<&str>, catetogry_orderv: u8) -> Vec<u8> {
    let kind_values: Vec<u8> = picked.iter().map(|&c| kind_to_order(c)).collect();
    let mut value: Vec<u8> = vec![catetogry_orderv];
    value.extend_from_slice(&kind_values);
    value
}

/// Decide the category of the input
fn check_same_kinds(sorted_kinds: &Vec<&str>, category: Category) -> bool {
    match category {
        Category::FourOfAKind => sorted_kinds[0..4].iter().all(|&c| c == sorted_kinds[0]),
        Category::FullHouse => {
            sorted_kinds[0..3].iter().all(|&c| c == sorted_kinds[0])
                && sorted_kinds[3..=4].iter().all(|&c| c == sorted_kinds[3])
        }
        Category::ThreeOfAKind => sorted_kinds[0..3].iter().all(|&c| c == sorted_kinds[0]),
        Category::TwoPairs => {
            sorted_kinds[0] == sorted_kinds[1] && sorted_kinds[2] == sorted_kinds[3]
        }
        Category::Pair => sorted_kinds[0] == sorted_kinds[1],
        _ => false,
    }
}

/// Search for flush cards from the 7.
/// This fn accept sorted-by-kind or unsorted cards (preferable).
/// It returns sorted-by-kind cards anyway.
fn find_flush<'a>(cards: &Vec<&'a str>) -> (bool, Vec<&'a str>) {
    let mut groups: HashMap<&'a str, Vec<&'a str>> = HashMap::with_capacity(7);

    for card in cards {
        let (suit, _) = card.split_at(1);
        groups
            .entry(suit)
            .and_modify(|grp| grp.push(card))
            .or_insert(vec![card]);
    }

    for (_, mut val) in groups.into_iter() {
        if val.len() >= 5 {
            val.sort_by(|&c1, &c2| compare_kinds(c1, c2));
            return (true, val.clone());
        }
    }

    (false, vec![])
}

const POSSIBLE_STRAIGHTS_ORDERS: [[u8; 5]; 10] = [
    [14, 13, 12, 11, 10],
    [13, 12, 11, 10, 9],
    [12, 11, 10, 9, 8],
    [11, 10, 9, 8, 7],
    [10, 9, 8, 7, 6],
    [9, 8, 7, 6, 5],
    [8, 7, 6, 5, 4],
    [7, 6, 5, 4, 3],
    [6, 5, 4, 3, 2],
    [5, 4, 3, 2, 14],
];

fn find_straights<'a>(cards: &Vec<&'a str>) -> (bool, Vec<Vec<&'a str>>) {
    let order_to_cards = |o: u8| {
        cards
            .iter()
            .filter(|c| kind_to_order(c) == o)
            .map(|c| *c)
            .collect::<Vec<&str>>()
    };

    let mut results = Vec::new();
    for orders in POSSIBLE_STRAIGHTS_ORDERS {
        let cards_vec = orders
            .iter()
            .map(|o| order_to_cards(*o))
            .collect::<Vec<Vec<&str>>>();

        for ca in cards_vec[0].iter() {
            for cb in cards_vec[1].iter() {
                for cc in cards_vec[2].iter() {
                    for cd in cards_vec[3].iter() {
                        for ce in cards_vec[4].iter() {
                            results.push(vec![*ca, *cb, *cc, *cd, *ce])
                        }
                    }
                }
            }
        }
    }

    if results.is_empty() {
        (false, results)
    } else {
        (true, results)
    }
}


/// This fn accepts either sorted-by-kind or unsorted cards (preferable).
/// It returns sorted-by-kind cards anyway.
fn find_royal_flush<'a>(cards: &Vec<&'a str>) -> (bool, Vec<&'a str>) {
    let royal_flush: [[&str; 5]; 4] = [
        ["ca", "ck", "cq", "cj", "ct"],
        ["da", "dk", "dq", "dj", "dt"],
        ["ha", "hk", "hq", "hj", "ht"],
        ["sa", "sk", "sq", "sj", "st"],
    ];

    let cards_set = HashSet::from([
        cards[0], cards[1], cards[2], cards[3], cards[4], cards[5], cards[6],
    ]);

    for rf in royal_flush {
        let royal_set = HashSet::from(rf);
        let mut hit: Vec<&str> = royal_set.intersection(&cards_set).map(|&c| c).collect();

        if hit.len() == 5 {
            hit.sort_by(|c1, c2| compare_kinds(c1, c2));
            return (true, hit);
        }
    }

    (false, vec![])
}

/// Search for straight flush from all found straights and flushes
fn find_straight_flush<'a>(
    flush: &Vec<&'a str>,
    straights: &Vec<Vec<&'a str>>,
) -> Vec<Vec<&'a str>> {
    // [9,8,7,6,5,4,3]
    // [7,6,5,4,3,2,14]
    let flush_set: HashSet<&str> = flush.iter().map(|&c| c).collect();
    let mut result: Vec<Vec<&str>> = Vec::new(); // with_capacity(3)?

    for straight in straights {
        let straight_set: HashSet<&str> = straight.iter().map(|&c| c).collect();
        let mut hit: Vec<&str> = straight_set.intersection(&flush_set).map(|&c| c).collect();
        hit.sort_by(|c1, c2| compare_kinds(c1, c2));
        if hit.len() == 5 {
            // Simply move A to the end
            if hit[0].contains("a") {
                let ace: &str = hit.remove(0);
                hit.push(ace);
            }
            result.push(hit)
        }
    }
    result
}

/// Compare values of two hands
pub fn compare_hands(handv1: &Vec<u8>, handv2: &Vec<u8>) -> Ordering {
    let result: Vec<i8> = handv1
        .iter()
        .zip(handv2.iter())
        .map(|(v1, v2)| -> i8 {
            if v1 > v2 {
                1
            } else if v1 < v2 {
                -1
            } else {
                0
            }
        })
        .filter(|&r| r != 0)
        .collect();

    if result.len() == 0 {
        // Two hands are equal
        Ordering::Equal
    } else if result[0] == 1 {
        Ordering::Greater
    } else {
        Ordering::Less
    }
}

/// This fn accpets unsorted cards.
pub fn evaluate_cards(cards: Vec<&str>) -> PlayerHand {
    let sorted_by_group: Vec<&str> = sort_suited_cards(&cards);
    let sorted_kinds: Vec<&str> = sorted_by_group
        .iter()
        .map(|&c| -> &str {
            let (_, k) = c.split_at(1);
            k
        })
        .collect();
    let (has_royal, rflush) = find_royal_flush(&cards);
    let (has_flush, flush_cards) = find_flush(&cards);

    let mut sorted_cards: Vec<&str> = cards.iter().map(|c| *c).collect();
    sorted_cards.sort_by(|&c1, &c2| compare_kinds(c1, c2));
    let (has_straights, straights) = find_straights(&sorted_cards);
    let sflush = find_straight_flush(&flush_cards, &straights);

    // royal flush
    if has_royal {
        let value = tag_value(&rflush, 9);
        PlayerHand {
            category: Category::RoyalFlush,
            picks: rflush.iter().map(|s| s.to_string()).collect(),
            value,
        }
    }
    // straight flush
    else if !sflush.is_empty() {
        let picks = sflush[0].to_vec();
        let value = tag_value(&picks, 8);
        PlayerHand {
            category: Category::StraightFlush,
            picks: picks.iter().map(|s| s.to_string()).collect(),
            value,
        }
    }
    // four of a kind
    else if check_same_kinds(&sorted_kinds, Category::FourOfAKind) {
        let picks = sorted_by_group[0..5].to_vec();
        let value = tag_value(&picks, 7);
        PlayerHand {
            category: Category::FourOfAKind,
            picks: picks.iter().map(|s| s.to_string()).collect(),
            value,
        }
    }
    // full house
    else if check_same_kinds(&sorted_kinds, Category::FullHouse) {
        let picks = sorted_by_group[0..5].to_vec();
        let value = tag_value(&picks, 6);
        PlayerHand {
            category: Category::FullHouse,
            picks: picks.iter().map(|s| s.to_string()).collect(),
            value,
        }
    }
    // flush
    else if has_flush {
        let picks = flush_cards[0..5].to_vec();
        let value = tag_value(&picks, 5);
        PlayerHand {
            category: Category::Flush,
            picks: picks.iter().map(|s| s.to_string()).collect(),
            value,
        }
    }
    // straight
    else if has_straights {
        let picks = straights[0].to_vec();
        let value = tag_value(&picks, 4);
        PlayerHand {
            category: Category::Straight,
            picks: picks.iter().map(|s| s.to_string()).collect(),
            value,
        }
    }
    // three of a kind
    else if check_same_kinds(&sorted_kinds, Category::ThreeOfAKind) {
        let picks = sorted_by_group[0..5].to_vec();
        let value = tag_value(&picks, 3);
        PlayerHand {
            category: Category::ThreeOfAKind,
            picks: picks.iter().map(|s| s.to_string()).collect(),
            value,
        }
    }
    // two pairs
    else if check_same_kinds(&sorted_kinds, Category::TwoPairs) {
        println!("{:?}", sorted_by_group);
        let mut picks = sorted_by_group[0..4].to_vec();
        let kicker = find_kicker(&sorted_by_group[4..]);
        picks.push(kicker);
        let value = tag_value(&picks, 2);
        PlayerHand {
            category: Category::TwoPairs,
            picks: picks.iter().map(|s| s.to_string()).collect(),
            value,
        }
    }
    // pair
    else if check_same_kinds(&sorted_kinds, Category::Pair) {
        let picks = sorted_by_group[0..5].to_vec();
        let value = tag_value(&picks, 1);
        PlayerHand {
            category: Category::Pair,
            picks: picks.iter().map(|s| s.to_string()).collect(),
            value,
        }
    }
    // high card
    else {
        let picks = sorted_by_group[0..5].to_vec();
        let value = tag_value(&picks, 0);
        PlayerHand {
            category: Category::HighCard,
            picks: picks.iter().map(|s| s.to_string()).collect(),
            value,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cmp::Ordering;

    #[test]
    fn sorting_cards() {
        // A single card is a 2-char string literal: Suit-Kind
        // For example, "hq" represents Heart Queen
        let community_cards: [&str; 5] = ["sa", "c2", "c7", "h2", "d5"];
        let hand: [&str; 2] = ["ca", "c4"]; // pair A
        let mut cards = create_cards(&community_cards, &hand);
        cards.sort_by(|&c1, &c2| compare_kinds(c1, c2));
        // Test sorted cards
        assert!(validate_cards(&cards));
        assert_eq!("ca", cards[1]);
        assert_eq!(vec!["sa", "ca", "c7", "d5", "c4", "c2", "h2"], cards);

        // Test sorting cards by grouped-kinds
        let sorted_cards = sort_suited_cards(&cards);
        assert_eq!(7, sorted_cards.len());
        assert_eq!(vec!["sa", "ca", "c2", "h2", "c7", "d5", "c4"], sorted_cards);
    }

    #[test]
    fn test_flush() {
        // Test flush
        let hole_card: [&str; 2] = ["d2", "h9"];
        let board: [&str; 5] = ["da", "dt", "c7", "d6", "d5"];
        let cards = create_cards(&board, &hole_card);
        assert!(validate_cards(&cards));

        let (has_flush, flush_cards) = find_flush(&cards);
        assert!(has_flush);
        assert_eq!(5, flush_cards.len());
        assert_eq!(vec!["da", "dt", "d6", "d5", "d2"], flush_cards);
    }

    #[test]
    fn test_straights() {
        // Test one normal straight: two _6 cards lead to 2 straights
        // ["d9", "d8", "c7", "d6", "s5"] and ["d9", "d8", "c7", "h6", "s5"]
        let hole_cards1: [&str; 2] = ["s5", "h6"];
        let board1: [&str; 5] = ["ca", "d6", "c7", "d8", "d9"];
        let mut cards1 = create_cards(&board1, &hole_cards1);
        cards1.sort_by(|&c1, &c2| compare_kinds(c1, c2));

        let (has_straights1, straights1) = find_straights(&cards1);
        assert!(has_straights1);
        assert_eq!(2, straights1.len());
        assert_eq!(vec!["d9", "d8", "c7", "d6", "s5"], straights1[0]);
        assert_eq!(vec!["d9", "d8", "c7", "h6", "s5"], straights1[1]);

        // Test three straights: [10,9,8,7,6,5,4]
        let hole_cards2: [&str; 2] = ["st", "h9"];
        let board2: [&str; 5] = ["c6", "d5", "c7", "d8", "d4"];
        let mut cards2 = create_cards(&board2, &hole_cards2);
        cards2.sort_by(|c1, c2| compare_kinds(c1, c2));

        let (has_straights2, straights2) = find_straights(&cards2);
        assert!(has_straights2);
        assert_eq!(3, straights2.len());
        assert_eq!(vec!["st", "h9", "d8", "c7", "c6"], straights2[0]);
        assert_eq!(vec!["h9", "d8", "c7", "c6", "d5"], straights2[1]);
        assert_eq!(vec!["d8", "c7", "c6", "d5", "d4"], straights2[2]);

        // Test A hight straight [14,13,12,11,10]
        let hole_cards3: [&str; 2] = ["sa", "hq"];
        let board3: [&str; 5] = ["cj", "dt", "ck", "sk", "hk"];
        let mut cards3 = create_cards(&board3, &hole_cards3);
        cards3.sort_by(|c1, c2| compare_kinds(c1, c2));

        let (has_straights3, straights3) = find_straights(&cards3);
        assert!(has_straights3);
        assert_eq!(3, straights3.len());
        assert_eq!(vec!["sa", "ck", "hq", "cj", "dt"], straights3[0]);
        assert_eq!(vec!["sa", "sk", "hq", "cj", "dt"], straights3[1]);
        assert_eq!(vec!["sa", "hk", "hq", "cj", "dt"], straights3[2]);

        // Test Five high straight [14,5,4,3,2]
        let hole_cards4: [&str; 2] = ["sa", "h7"];
        let board4: [&str; 5] = ["c5", "d3", "c2", "ha", "d4"];
        let mut cards4 = create_cards(&board4, &hole_cards4);
        cards4.sort_by(|c1, c2| compare_kinds(c1, c2));

        let (has_straights4, straights4) = find_straights(&cards4);
        assert!(has_straights4);
        assert_eq!(2, straights4.len());
        assert_eq!(vec!["c5", "d4", "d3", "c2", "ha"], straights4[0]);
        assert_eq!(vec!["c5", "d4", "d3", "c2", "sa"], straights4[1]);

        // Test Four of a kind or full house (this is by accident)
        let hole_cards5: [&str; 2] = ["sa", "h7"];
        let board5: [&str; 5] = ["ca", "d7", "c2", "ha", "d4"];
        let mut cards5 = create_cards(&board5, &hole_cards5);
        cards5.sort_by(|c1, c2| compare_kinds(c1, c2));

        let (has_straights5, _straights5) = find_straights(&cards5);
        assert!(!has_straights5);
    }

    #[test]
    fn test_fullhouse() {
        let hole_cards: [&str; 2] = ["sa", "h7"];
        let board: [&str; 5] = ["ca", "d7", "c2", "ha", "d4"];
        let result = evaluate_cards(create_cards(&board, &hole_cards));
        assert_eq!(result.category, Category::FullHouse);
    }

    #[test]
    fn test_four_of_a_kind() {
        let hole_cards: [&str; 2] = ["sa", "h7"];
        let board: [&str; 5] = ["ca", "d7", "da", "ha", "d4"];
        let result = evaluate_cards(create_cards(&board, &hole_cards));
        assert_eq!(result.category, Category::FourOfAKind);
    }

    #[test]
    fn test_two_pairs() {
        let hole_cards: [&str; 2] = ["c9", "dt"];
        let board: [&str; 5] = ["hq", "cq", "dk", "d9", "ct"];
        let result = evaluate_cards(create_cards(&board, &hole_cards));
        assert_eq!(result.picks, vec!["hq", "cq", "ct", "dt", "dk"]);
        assert_eq!(result.value, vec![2, 12, 12, 10, 10, 13]);
        assert_eq!(result.category, Category::TwoPairs);

        let hole_cards: [&str; 2] = ["c9", "da"];
        let board: [&str; 5] = ["hq", "cq", "dk", "d9", "ct"];
        let result = evaluate_cards(create_cards(&board, &hole_cards));
        assert_eq!(result.picks, vec!["hq", "cq", "d9", "c9", "da"]);
        assert_eq!(result.value, vec![2, 12, 12, 9, 9, 14]);
        assert_eq!(result.category, Category::TwoPairs);
    }

    #[test]
    fn test_royal_flush() {
        let hole_cards: [&str; 2] = ["sa", "sq"];
        let board: [&str; 5] = ["sk", "hk", "hj", "sj", "st"];
        let mut cards = create_cards(&board, &hole_cards);
        cards.sort_by(|c1, c2| compare_kinds(c1, c2));

        let (has_rf, rf) = find_royal_flush(&cards);
        assert!(has_rf);
        assert_eq!(5, rf.len());
        assert_eq!(vec!["sa", "sk", "sq", "sj", "st"], rf);
    }

    #[test]
    fn test_straight_flush() {
        let hole_cards: [&str; 2] = ["ha", "h5"];
        let board: [&str; 5] = ["h7", "h6", "h2", "h3", "h4"];
        let mut cards = create_cards(&board, &hole_cards);
        cards.sort_by(|c1, c2| compare_kinds(c1, c2));

        let (has_f, flush) = find_flush(&cards);
        let (has_s, straights) = find_straights(&cards);
        let sf = find_straight_flush(&flush, &straights);

        assert!(has_f);
        assert!(has_s);
        assert_eq!(7, flush.len());
        assert_eq!(3, straights.len());
        assert_eq!(vec!["h7", "h6", "h5", "h4", "h3"], sf[0]);
        assert_eq!(vec!["h6", "h5", "h4", "h3", "h2"], sf[1]);
        assert_eq!(vec!["h5", "h4", "h3", "h2", "ha"], sf[2]);
    }

    #[test]
    fn test_pairs() {
        let hole_cards: [&str; 2] = ["ha", "h5"];
        let board: [&str; 5] = ["d7", "c6", "s7", "c7", "st"];
        let cards = create_cards(&board, &hole_cards);
        let sorted_by_group: Vec<&str> = sort_suited_cards(&cards);
        let sorted_kinds: Vec<&str> = sorted_by_group
            .iter()
            .map(|&c| -> &str {
                let (_, k) = c.split_at(1);
                k
            })
            .collect();

        assert!(check_same_kinds(&sorted_kinds, Category::ThreeOfAKind));
    }

    #[test]
    fn test_evaluator() {
        let hole_cards: [&str; 2] = ["ha", "h5"];
        let board: [&str; 5] = ["d7", "c6", "s7", "c7", "st"];
        let cards = create_cards(&board, &hole_cards);

        let result: PlayerHand = evaluate_cards(cards);
        assert_eq!(result.category, Category::ThreeOfAKind);
        assert_eq!(vec!["d7", "s7", "c7", "ha", "st"], result.picks);
        assert_eq!(vec![3, 7, 7, 7, 14, 10], result.value);

        let hole_cards1: [&str; 2] = ["c4", "hk"];
        let hole_cards2: [&str; 2] = ["sa", "d9"];
        let board: [&str; 5] = ["d2", "da", "s2", "h3", "h5"];
        let cards1 = create_cards(&board, &hole_cards1);
        let cards2 = create_cards(&board, &hole_cards2);

        let result1: PlayerHand = evaluate_cards(cards1);
        assert_eq!(result1.category, Category::Straight);
        let result2: PlayerHand = evaluate_cards(cards2);
        assert_eq!(result2.category, Category::TwoPairs);
    }

    #[test]
    fn test_compare_hands() {
        let hole_cards1: [&str; 2] = ["h7", "h5"]; // FullHouse
        let hole_cards2: [&str; 2] = ["s2", "d8"]; // two pairs
        let board: [&str; 5] = ["d7", "c6", "s6", "c7", "st"];
        let cards1 = create_cards(&board, &hole_cards1);
        let cards2 = create_cards(&board, &hole_cards2);
        let hand1: PlayerHand = evaluate_cards(cards1);
        let hand2: PlayerHand = evaluate_cards(cards2);

        // Test detail of two hands
        assert_eq!(Category::FullHouse, hand1.category);
        assert_eq!(Category::TwoPairs, hand2.category);
        assert_eq!(vec!["d7", "c7", "h7", "c6", "s6"], hand1.picks);
        assert_eq!(vec!["d7", "c7", "c6", "s6", "st"], hand2.picks);
        assert_eq!(vec![6, 7, 7, 7, 6, 6], hand1.value);
        assert_eq!(vec![2, 7, 7, 6, 6, 10], hand2.value);

        // Test result
        let result = compare_hands(&hand1.value, &hand2.value);
        assert_eq!(Ordering::Greater, result);

        // Test two equal hands: both are 10 pair
        let hole_cards3: [&str; 2] = ["d9", "h4"];
        let hole_cards4: [&str; 2] = ["h9", "s4"];
        let cmt_cards2: [&str; 5] = ["st", "ht", "sk", "c8", "d5"];
        let cards3 = create_cards(&cmt_cards2, &hole_cards3);
        let cards4 = create_cards(&cmt_cards2, &hole_cards4);
        let hand3: PlayerHand = evaluate_cards(cards3);
        let hand4: PlayerHand = evaluate_cards(cards4);

        // Test the detail of the two hands
        assert_eq!(Category::Pair, hand3.category);
        assert_eq!(Category::Pair, hand4.category);
        assert_eq!(vec!["st", "ht", "sk", "d9", "c8"], hand3.picks);
        assert_eq!(vec!["st", "ht", "sk", "h9", "c8"], hand4.picks);
        assert_eq!(vec![1, 10, 10, 13, 9, 8], hand3.value);
        assert_eq!(vec![1, 10, 10, 13, 9, 8], hand4.value);

        // Test result
        let result = compare_hands(&hand3.value, &hand4.value);
        assert_eq!(Ordering::Equal, result);
    }
}
