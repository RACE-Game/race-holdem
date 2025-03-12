use super::*;

// Test sort ranks
#[test]
fn test_sort_ranks_ensures_eliminated_players_keep_order() {
    let mut mtt = Mtt::default();

    // Active players
    mtt.ranks
        .push(PlayerRank::new(1, 5000, PlayerRankStatus::Play, 0));
    mtt.ranks
        .push(PlayerRank::new(2, 10000, PlayerRankStatus::Play, 1));
    mtt.ranks
        .push(PlayerRank::new(3, 7000, PlayerRankStatus::Play, 2));

    // Eliminated players
    mtt.ranks
        .push(PlayerRank::new(4, 0, PlayerRankStatus::Out, 3));
    mtt.ranks
        .push(PlayerRank::new(5, 0, PlayerRankStatus::Out, 4));

    mtt.sort_ranks();

    assert_eq!(mtt.ranks[0].id, 2);
    assert_eq!(mtt.ranks[1].id, 3);
    assert_eq!(mtt.ranks[2].id, 1);
    assert_eq!(mtt.ranks[3].id, 4);
    assert_eq!(mtt.ranks[4].id, 5);
}
