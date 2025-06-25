use crate::{BlindInfo, MttAccountData};

use super::*;

#[test]
fn test_init_state_valid() {
    let data = MttAccountData {
        start_time: 1000,
        entry_close_time: 2000,
        ticket: 100,
        table_size: 9,
        start_chips: 5000,
        early_bird_chips: 1000,
        blind_info: BlindInfo::default(),
        prize_rules: vec![PrizeRule {
            min_players: 0, prizes: vec![50, 30, 20]
        }],
        min_players: 2,
        rake: 50,
        bounty: 20,
        theme: None,
        subgame_bundle: "holdem".to_string(),
    };

    let init_account = InitAccount {
        max_players: 100,
        data: borsh::to_vec(&data).unwrap(),
    };

    let mut e = Effect::default();
    let result = Mtt::init_state(&mut e, init_account);
    assert!(result.is_ok());
    let state = result.unwrap();
    assert_eq!(state.start_time, 1000);
    assert_eq!(state.entry_close_time, 2000);
    assert_eq!(state.ticket, 100);
    assert_eq!(state.table_size, 9);
    assert_eq!(state.start_chips, 5000);
    assert_eq!(state.prize_rules[0].prizes, vec![50, 30, 20]);
    assert_eq!(state.min_players, 2);
    assert_eq!(state.rake, 50);
    assert_eq!(state.bounty, 20);
}

#[test]
fn test_init_state_invalid_prize_rules() {
    let data = MttAccountData {
        prize_rules: vec![PrizeRule {
            min_players: 0,
            prizes: vec![50, 30], // Doesn't sum to 100
        }],
        ..Default::default()
    };
    let init_account = InitAccount {
        max_players: 100,
        data: borsh::to_vec(&data).unwrap(),
    };

    let mut e = Effect::default();
    let result = Mtt::init_state(&mut e, init_account);
    assert!(result.is_err());
}

#[test]
fn test_init_state_invalid_entry_close_time() {
    let data = MttAccountData {
        start_time: 2000,
        entry_close_time: 1000, // Earlier than start_time
        ..Default::default()
    };
    let init_account = InitAccount {
        max_players: 100,
        data: borsh::to_vec(&data).unwrap(),
    };

    let mut e = Effect::default();
    let result = Mtt::init_state(&mut e, init_account);
    assert!(result.is_err());
}

#[test]
fn test_init_state_invalid_rake_bounty() {
    let data = MttAccountData {
        rake: 600,
        bounty: 500, // Sum > 1000
        ..Default::default()
    };
    let init_account = InitAccount {
        max_players: 100,
        data: borsh::to_vec(&data).unwrap(),
    };

    let mut e = Effect::default();
    let result = Mtt::init_state(&mut e, init_account);
    assert!(result.is_err());
}

#[test]
fn test_init_state_start_chips_too_low() {
    let data = MttAccountData {
        start_chips: 100, // Too low for 50BB
        blind_info: BlindInfo {
            blind_rules: vec![BlindRuleItem::new(5, 10, 0)], // 50BB would be 500
            ..Default::default()
        },
        ..Default::default()
    };
    let init_account = InitAccount {
        max_players: 100,
        data: borsh::to_vec(&data).unwrap(),
    };

    let mut e = Effect::default();
    let result = Mtt::init_state(&mut e, init_account);
    assert!(result.is_err());
}
