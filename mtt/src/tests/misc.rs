use super::*;

#[test]
fn test_update_player_balances() {
    let mut mtt = Mtt::default();
    mtt.total_prize = 10000;
    mtt.total_rake = 2000;
    mtt.update_player_balances();
    assert_eq!(mtt.player_balances.get(&0), Some(&12000));
}

// Test sort ranks
#[test]
fn test_sort_ranks_ensures_eliminated_players_keep_order() {
    let mut mtt = Mtt::default();

    // Active players
    mtt.ranks
        .push(PlayerRank::new(1, 5000, PlayerRankStatus::Play, 0, vec![5000], DEFAULT_TIME_CARDS));
    mtt.ranks
        .push(PlayerRank::new(2, 10000, PlayerRankStatus::Play, 1, vec![10000], DEFAULT_TIME_CARDS));
    mtt.ranks
        .push(PlayerRank::new(3, 7000, PlayerRankStatus::Play, 2, vec![7000], DEFAULT_TIME_CARDS));

    // Eliminated players
    mtt.ranks
        .push(PlayerRank::new(4, 0, PlayerRankStatus::Out, 3, vec![5000], DEFAULT_TIME_CARDS));
    mtt.ranks
        .push(PlayerRank::new(5, 0, PlayerRankStatus::Out, 4, vec![5000], DEFAULT_TIME_CARDS));

    mtt.sort_ranks();

    assert_eq!(mtt.ranks[0].id, 2);
    assert_eq!(mtt.ranks[1].id, 3);
    assert_eq!(mtt.ranks[2].id, 1);
    assert_eq!(mtt.ranks[3].id, 4);
    assert_eq!(mtt.ranks[4].id, 5);
}


#[test]
fn test_get_table_ids_with_least_most_players() {
    let mtt = Mtt {
        tables: BTreeMap::from([
            (
                1,
                MttTableState {
                    table_id: 1,
                    players: vec![
                        MttTablePlayer::new_with_defaults(1, 10000, 0),
                        MttTablePlayer::new_with_defaults(2, 10000, 1),
                        MttTablePlayer::new_with_defaults(3, 10000, 2),
                    ],
                    ..Default::default()
                },
            ),
            (
                2,
                MttTableState {
                    table_id: 2,
                    players: vec![
                        MttTablePlayer::new_with_defaults(1, 10000, 0),
                        MttTablePlayer::new_with_defaults(2, 10000, 1),
                    ],
                    ..Default::default()
                },
            ),
            (
                3,
                MttTableState {
                    table_id: 3,
                    players: vec![
                        MttTablePlayer::new_with_defaults(1, 10000, 0),
                        MttTablePlayer::new_with_defaults(1, 10000, 1),
                        MttTablePlayer::new_with_defaults(1, 10000, 2),
                        MttTablePlayer::new_with_defaults(2, 10000, 3),
                     ],
                    ..Default::default()
                },
            ),
            (
                4,
                MttTableState {
                    table_id: 4,
                    players: vec![
                        MttTablePlayer::new_with_defaults(1, 10000, 0),
                        MttTablePlayer::new_with_defaults(2, 10000, 1),
                    ],
                    ..Default::default()
                },
            ),
            (
                5,
                MttTableState {
                    table_id: 5,
                    players: vec![
                        MttTablePlayer::new_with_defaults(1, 10000, 0),
                        MttTablePlayer::new_with_defaults(1, 10000, 1),
                        MttTablePlayer::new_with_defaults(1, 10000, 2),
                        MttTablePlayer::new_with_defaults(2, 10000, 3),
                    ],
                    ..Default::default()
                },
            ),
        ]),
        ..Default::default()
    };

    let (least, most) = mtt.get_table_ids_with_least_most_players(1).unwrap();
    assert_eq!(least.table_id, 2);
    assert_eq!(most.table_id, 3);

    let (least, most) = mtt.get_table_ids_with_least_most_players(2).unwrap();
    assert_eq!(least.table_id, 2);
    assert_eq!(most.table_id, 3);

    let (least, most) = mtt.get_table_ids_with_least_most_players(3).unwrap();
    assert_eq!(least.table_id, 2);
    assert_eq!(most.table_id, 3);

    let (least, most) = mtt.get_table_ids_with_least_most_players(4).unwrap();
    assert_eq!(least.table_id, 4);
    assert_eq!(most.table_id, 3);

    let (least, most) = mtt.get_table_ids_with_least_most_players(5).unwrap();
    assert_eq!(least.table_id, 2);
    assert_eq!(most.table_id, 5);
}


#[test]
pub fn test_available_seats() -> anyhow::Result<()> {
    let mut mtt = Mtt::default();
    mtt.table_size = 6;
    let players = vec![
        MttTablePlayer::new(1, 1000, 1, 0),
        MttTablePlayer::new(2, 1000, 2, 0),
        MttTablePlayer::new(3, 1000, 4, 0),
        MttTablePlayer::new(4, 1000, 5, 0),
    ];
    mtt.tables.insert(1, MttTableState::new(1, 10, 20, 0, players));
    assert_eq!(mtt.find_available_table_seats(1)?, vec![0,3]);
    return Ok(())
}
