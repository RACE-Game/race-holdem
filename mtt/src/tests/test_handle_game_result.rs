use super::*;

// ----------------------
// Update table tests
// ----------------------

#[test]
fn test_get_table_ids_with_least_most_players() {
    let mtt = Mtt {
        tables: BTreeMap::from([
            (
                1,
                MttTableState {
                    table_id: 1,
                    players: vec![
                        MttTablePlayer::new(1, 10000, 0),
                        MttTablePlayer::new(2, 10000, 1),
                        MttTablePlayer::new(3, 10000, 2),
                    ],
                    ..Default::default()
                },
            ),
            (
                2,
                MttTableState {
                    table_id: 2,
                    players: vec![
                        MttTablePlayer::new(1, 10000, 0),
                        MttTablePlayer::new(2, 10000, 1),
                    ],
                    ..Default::default()
                },
            ),
            (
                3,
                MttTableState {
                    table_id: 3,
                    players: vec![
                        MttTablePlayer::new(1, 10000, 0),
                        MttTablePlayer::new(1, 10000, 0),
                        MttTablePlayer::new(1, 10000, 0),
                        MttTablePlayer::new(2, 10000, 1),
                    ],
                    ..Default::default()
                },
            ),
            (
                4,
                MttTableState {
                    table_id: 4,
                    players: vec![
                        MttTablePlayer::new(1, 10000, 0),
                        MttTablePlayer::new(2, 10000, 1),
                    ],
                    ..Default::default()
                },
            ),
            (
                5,
                MttTableState {
                    table_id: 5,
                    players: vec![
                        MttTablePlayer::new(1, 10000, 0),
                        MttTablePlayer::new(1, 10000, 0),
                        MttTablePlayer::new(1, 10000, 0),
                        MttTablePlayer::new(2, 10000, 1),
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
fn test_game_result_given_3_tables_and_current_table_has_1_player_do_dispatch_nothing() {
    // Create three tables with number of players: 3, 3, 2
    let mut mtt = helper::create_mtt_with_players(&[3, 3, 2], 3);
    let mut effect = Effect::default();

    // Player 7 lost to player 8, thus there's only one player on the table
    // Current player distribution: 3, 3, 1
    // Becasue all tables are full, we expect no player is moved
    let game_result = HoldemBridgeEvent::GameResult {
        hand_id: 1,
        table_id: 3,
        chips_change: BTreeMap::from([(7, ChipsChange::Sub(10000)), (8, ChipsChange::Add(10000))]),
        table: MttTableState {
            hand_id: 1,
            table_id: 3,
            players: vec![MttTablePlayer::new(8, 20000, 1)],
            next_game_start: 0,
            ..Default::default()
        },
    };
    let game_result_event = Event::Bridge {
        dest_game_id: 0, // the table with two players
        from_game_id: 3,
        raw: borsh::to_vec(&game_result).unwrap(),
    };

    mtt.handle_event(&mut effect, game_result_event).unwrap();

    assert_eq!(mtt.get_rank(7).unwrap().chips, 0);
    assert_eq!(mtt.get_rank(8).unwrap().chips, 20000);
    assert_eq!(effect.bridge_events, vec![]);
}

#[test]
fn test_game_result_given_2_tables_and_current_table_has_1_player_do_dispatch_nothing() {
    // Create three tables with number of players: 3, 2
    let mut mtt = helper::create_mtt_with_players(&[3, 2], 3);
    let mut effect = Effect::default();

    let game_result = HoldemBridgeEvent::GameResult {
        hand_id: 1,
        table_id: 2,
        chips_change: BTreeMap::from([(4, ChipsChange::Sub(10000)), (5, ChipsChange::Add(10000))]),
        table: MttTableState {
            hand_id: 1,
            table_id: 2,
            players: vec![MttTablePlayer::new(5, 20000, 1)],
            next_game_start: 0,
            ..Default::default()
        },
    };
    let game_result_event = Event::Bridge {
        dest_game_id: 0, // the table with two players
        from_game_id: 2,
        raw: borsh::to_vec(&game_result).unwrap(),
    };

    mtt.handle_event(&mut effect, game_result_event).unwrap();

    assert_eq!(mtt.get_rank(4).unwrap().chips, 0);
    assert_eq!(mtt.get_rank(5).unwrap().chips, 20000);
    assert_eq!(effect.bridge_events, vec![]);
}

#[test]
fn test_game_result_given_3_tables_and_single_player_in_different_table_do_dispatch_relocate_and_start_game(
) {
    // Create three tables with number of players: 3, 3, 1
    let mut mtt = helper::create_mtt_with_players(&[3, 3, 1], 3);
    let mut effect = Effect::default();

    // Whatever happened on current table should trigger a table balancing
    let game_result = HoldemBridgeEvent::GameResult {
        hand_id: 1,
        table_id: 2,
        chips_change: BTreeMap::default(),
        table: MttTableState {
            hand_id: 1,
            table_id: 2,
            players: vec![
                MttTablePlayer::new(4, 10000, 0),
                MttTablePlayer::new(5, 10000, 1),
                MttTablePlayer::new(6, 10000, 2),
            ],
            ..Default::default()
        },
    };

    let game_result_event = Event::Bridge {
        dest_game_id: 0,
        from_game_id: 2,
        raw: borsh::to_vec(&game_result).unwrap(),
    };

    mtt.handle_event(&mut effect, game_result_event).unwrap();

    assert_eq!(mtt.tables.get(&1).unwrap().players.len(), 3);
    assert_eq!(mtt.tables.get(&2).unwrap().players.len(), 3);
    assert_eq!(mtt.tables.get(&3).unwrap().players.len(), 1);
    assert_eq!(
        effect.list_bridge_events().unwrap(),
        vec![(
            2,
            HoldemBridgeEvent::StartGame {
                sb: DEFAULT_SB,
                bb: DEFAULT_BB,
                sitout_players: vec![4],
            },
        )]
    );
}

#[test]
fn test_game_result_given_3_tables_do_close_table() {
    // Create three tables with number of players: 3, 3, 1
    let mut mtt = helper::create_mtt_with_players(&[3, 3, 1], 3);
    let mut effect = Effect::default();

    // Eliminate one player on table #1
    // Player distribution becomes 2, 3, 1
    // Now it's possible to close current table, and move all players to table #3
    let game_result = HoldemBridgeEvent::GameResult {
        hand_id: 1,
        table_id: 1,
        chips_change: BTreeMap::from([(1, ChipsChange::Add(10000)), (2, ChipsChange::Sub(10000))]),
        table: MttTableState {
            hand_id: 1,
            table_id: 1,
            players: vec![
                MttTablePlayer::new(0, 20000, 0),
                MttTablePlayer::new(3, 10000, 2),
            ],
            ..Default::default()
        },
    };

    let game_result_event = Event::Bridge {
        dest_game_id: 0,
        from_game_id: 1,
        raw: borsh::to_vec(&game_result).unwrap(),
    };

    mtt.handle_event(&mut effect, game_result_event).unwrap();

    // Table 1 should be closed, two players should be moved to table 3
    assert_eq!(mtt.tables.len(), 3);
    assert_eq!(mtt.get_rank(2).unwrap().chips, 0);
    assert_eq!(mtt.get_rank(1).unwrap().chips, 20000);
    assert_eq!(mtt.tables.get(&1).unwrap().players.len(), 2);
    assert_eq!(mtt.tables.get(&2).unwrap().players.len(), 3);
    assert_eq!(mtt.tables.get(&3).unwrap().players.len(), 1);
    assert_eq!(
        effect.list_bridge_events().unwrap(),
        vec![(1, HoldemBridgeEvent::CloseTable),]
    );
}

#[test]
fn test_game_result_given_2_tables_do_close_table() {
    // Create three tables with number of players: 2, 2
    let mut mtt = helper::create_mtt_with_players(&[2, 2], 3);
    let mut effect = Effect::default();

    // Eliminate one player on table #1
    // Players distribution becomes 1, 2
    // The tables should be merged
    let game_result = HoldemBridgeEvent::GameResult {
        hand_id: 1,
        table_id: 1,
        chips_change: BTreeMap::from([(1, ChipsChange::Add(10000)), (2, ChipsChange::Sub(10000))]),
        table: MttTableState {
            hand_id: 1,
            table_id: 1,
            players: vec![MttTablePlayer::new(1, 20000, 0)],
            ..Default::default()
        },
    };

    let game_result_event = Event::Bridge {
        dest_game_id: 0,
        from_game_id: 1,
        raw: borsh::to_vec(&game_result).unwrap(),
    };

    mtt.handle_event(&mut effect, game_result_event).unwrap();

    // Table 1 should be closed, two players should be moved to table 3
    // assert_eq!(mtt.tables.len(), 2);
    assert_eq!(mtt.get_rank(2).unwrap().chips, 0);
    assert_eq!(mtt.get_rank(1).unwrap().chips, 20000);
    assert_eq!(
        effect.list_bridge_events().unwrap(),
        vec![(1, HoldemBridgeEvent::CloseTable),]
    );
}

#[test]
fn test_game_result_given_2_tables_move_one_player() {
    // Create three tables with number of players: 3, 1
    let mut mtt = helper::create_mtt_with_players(&[3, 1], 3);
    let mut effect = Effect::default();

    let game_result = HoldemBridgeEvent::GameResult {
        hand_id: 1,
        table_id: 1,
        chips_change: BTreeMap::default(),
        table: MttTableState {
            hand_id: 1,
            table_id: 1,
            players: vec![
                MttTablePlayer::new(1, 10000, 0),
                MttTablePlayer::new(2, 10000, 1),
                MttTablePlayer::new(3, 10000, 2),
            ],
            ..Default::default()
        },
    };

    let game_result_event = Event::Bridge {
        dest_game_id: 0,
        from_game_id: 1,
        raw: borsh::to_vec(&game_result).unwrap(),
    };

    mtt.handle_event(&mut effect, game_result_event).unwrap();

    assert_eq!(
        effect.list_bridge_events().unwrap(),
        vec![(
            1,
            HoldemBridgeEvent::StartGame {
                sitout_players: vec![1],
                sb: DEFAULT_SB,
                bb: DEFAULT_BB,
            }
        ),]
    );
    assert_eq!(mtt.table_assigns.get(&1), Some(&1));
    assert_eq!(mtt.table_assigns.get(&2), Some(&1));
    assert_eq!(mtt.table_assigns.get(&3), Some(&1));
    assert_eq!(mtt.table_assigns.get(&4), Some(&2));
    assert_eq!(mtt.tables.get(&1).map(|t| t.players.len()), Some(3));
    assert_eq!(mtt.tables.get(&2).map(|t| t.players.len()), Some(1));
}
