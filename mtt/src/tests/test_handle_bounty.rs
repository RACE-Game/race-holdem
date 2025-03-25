use race_api::effect::Withdraw;

use super::*;

#[test]
fn test_handle_bounty_single_winner_divisible() {
    let mut mtt = Mtt::default();
    let mut effect = Effect::default();
    let player_results = vec![
        PlayerResult::new(1, 1000, Some(ChipsChange::Add(500)), PlayerResultStatus::Normal),
        PlayerResult::new(2, 0, Some(ChipsChange::Sub(500)), PlayerResultStatus::Eliminated),
    ];
    mtt.ranks.push(PlayerRank::new(1, 1000, PlayerRankStatus::Play, 0));
    mtt.ranks.push(PlayerRank::new(2, 0, PlayerRankStatus::Out, 1));
    mtt.ranks[1].bounty_reward = 100;
    mtt.ranks[1].bounty_transfer = 200;
    mtt.handle_bounty(&player_results, &mut effect).unwrap();
    assert_eq!(mtt.ranks[0].bounty_reward, 200);
    assert_eq!(mtt.total_prize, 0);
    assert_eq!(effect.withdraws, vec![
        Withdraw { player_id: 1, amount: 100 }
    ]);
}

#[test]
fn test_handle_bounty_single_winner_non_divisible() {
    let mut mtt = Mtt::default();
    let mut effect = Effect::default();
    let player_results = vec![
        PlayerResult::new(1, 1000, Some(ChipsChange::Add(500)), PlayerResultStatus::Normal),
        PlayerResult::new(2, 0, Some(ChipsChange::Sub(500)), PlayerResultStatus::Eliminated),
    ];
    mtt.ranks.push(PlayerRank::new(1, 1000, PlayerRankStatus::Play, 0));
    mtt.ranks.push(PlayerRank::new(2, 0, PlayerRankStatus::Out, 1));
    mtt.ranks[1].bounty_reward = 101;
    mtt.ranks[1].bounty_transfer = 203;
    mtt.handle_bounty(&player_results, &mut effect).unwrap();

    effect.print_logs();

    assert_eq!(mtt.ranks[0].bounty_reward, 203);
    assert_eq!(mtt.total_prize, 0);
    assert_eq!(effect.withdraws, vec![
        Withdraw { player_id: 1, amount: 101 }
    ]);
}

#[test]
fn test_handle_bounty_multiple_winners_divisible() {
    let mut mtt = Mtt::default();
    let mut effect = Effect::default();
    let player_results = vec![
        PlayerResult::new(1, 1000, Some(ChipsChange::Add(250)), PlayerResultStatus::Normal),
        PlayerResult::new(2, 1000, Some(ChipsChange::Add(250)), PlayerResultStatus::Normal),
        PlayerResult::new(3, 0, Some(ChipsChange::Sub(500)), PlayerResultStatus::Eliminated),
    ];
    mtt.ranks.push(PlayerRank::new(1, 1000, PlayerRankStatus::Play, 0));
    mtt.ranks.push(PlayerRank::new(2, 1000, PlayerRankStatus::Play, 1));
    mtt.ranks.push(PlayerRank::new(3, 0, PlayerRankStatus::Out, 2));
    mtt.ranks[2].bounty_reward = 100;
    mtt.ranks[2].bounty_transfer = 200;
    mtt.handle_bounty(&player_results, &mut effect).unwrap();

    effect.print_logs();

    assert_eq!(mtt.ranks[0].bounty_reward, 100);
    assert_eq!(mtt.ranks[1].bounty_reward, 100);
    assert_eq!(mtt.total_prize, 0);
    assert_eq!(effect.withdraws, vec![
        Withdraw { player_id: 1, amount: 50 },
        Withdraw { player_id: 2, amount: 50 },
    ]);
}

#[test]
fn test_handle_bounty_multiple_winners_non_divisible() {
    let mut mtt = Mtt::default();
    let mut effect = Effect::default();
    let player_results = vec![
        PlayerResult::new(1, 1000, Some(ChipsChange::Add(250)), PlayerResultStatus::Normal),
        PlayerResult::new(2, 1000, Some(ChipsChange::Add(250)), PlayerResultStatus::Normal),
        PlayerResult::new(3, 0, Some(ChipsChange::Sub(500)), PlayerResultStatus::Eliminated),
    ];
    mtt.ranks.push(PlayerRank::new(1, 1000, PlayerRankStatus::Play, 0));
    mtt.ranks.push(PlayerRank::new(2, 1000, PlayerRankStatus::Play, 1));
    mtt.ranks.push(PlayerRank::new(3, 0, PlayerRankStatus::Out, 2));
    mtt.ranks[2].bounty_reward = 101;
    mtt.ranks[2].bounty_transfer = 203;
    mtt.handle_bounty(&player_results, &mut effect).unwrap();

    effect.print_logs();

    // 50 -> player 1
    // 50 -> player 2
    // 101 add to bounty of player 1
    // 101 add to bounty of player 2
    // remain 2, add to prize

    assert_eq!(mtt.ranks[0].bounty_reward, 101);
    assert_eq!(mtt.ranks[1].bounty_reward, 101);
    assert_eq!(mtt.total_prize, 2);
    assert_eq!(effect.withdraws, vec![
        Withdraw { player_id: 1, amount: 50 },
        Withdraw { player_id: 2, amount: 50 },
    ]);
}

#[test]
fn test_handle_bounty_no_eliminated() {
    let mut mtt = Mtt::default();
    let mut effect = Effect::default();
    let player_results = vec![
        PlayerResult::new(1, 1000, Some(ChipsChange::Add(250)), PlayerResultStatus::Normal),
        PlayerResult::new(2, 1000, Some(ChipsChange::Add(250)), PlayerResultStatus::Normal),
    ];
    mtt.ranks.push(PlayerRank::new(1, 1000, PlayerRankStatus::Play, 0));
    mtt.ranks.push(PlayerRank::new(2, 1000, PlayerRankStatus::Play, 1));
    mtt.handle_bounty(&player_results, &mut effect).unwrap();
    assert_eq!(mtt.ranks[0].bounty_reward, 0);
    assert_eq!(mtt.ranks[1].bounty_reward, 0);
    assert_eq!(mtt.total_prize, 0);
    assert_eq!(effect.withdraws, vec![]);
}
