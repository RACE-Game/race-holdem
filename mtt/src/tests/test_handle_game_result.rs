use super::*;
use race_holdem_mtt_base::PlayerResultStatus;

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
        player_results: vec![
            PlayerResult::new(
                7,
                0,
                Some(ChipsChange::Sub(10000)),
                PlayerResultStatus::Eliminated,
            ),
            PlayerResult::new(
                8,
                20000,
                Some(ChipsChange::Add(10000)),
                PlayerResultStatus::Normal,
            ),
        ],
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
        player_results: vec![
            PlayerResult::new(
                4,
                0,
                Some(ChipsChange::Sub(10000)),
                PlayerResultStatus::Eliminated,
            ),
            PlayerResult::new(
                5,
                20000,
                Some(ChipsChange::Add(10000)),
                PlayerResultStatus::Normal,
            ),
        ],
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
        player_results: vec![],
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
    assert_eq!(mtt.tables.get(&2).unwrap().players.len(), 2);
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
        ),(
            3,
            HoldemBridgeEvent::SitinPlayers {
                sitins: vec![MttTableSitin::new(4, 10000)]
            }
        )]
    );
}

#[test]
fn test_game_result_given_3_tables_do_close_table() {
    // Create tables with number of players: 3, 3, 1
    let mut mtt = helper::create_mtt_with_players(&[3, 3, 1], 3);
    mtt.stage = MttStage::Playing;
    let mut effect = Effect::default();

    // Eliminate one player on table #1
    // Player distribution becomes 2, 3, 1
    // Now it's possible to close current table, and move all players to table #3
    let game_result = HoldemBridgeEvent::GameResult {
        hand_id: 1,
        table_id: 1,
        player_results: vec![
            PlayerResult::new(
                2,
                0,
                Some(ChipsChange::Sub(10000)),
                PlayerResultStatus::Eliminated,
            ),
            PlayerResult::new(
                1,
                20000,
                Some(ChipsChange::Add(10000)),
                PlayerResultStatus::Normal,
            ),
        ],
        table: MttTableState {
            hand_id: 1,
            table_id: 1,
            players: vec![
                MttTablePlayer::new(1, 20000, 0),
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

    effect.print_logs();

    // Table 1 should be closed, two players should be moved to table 3
    assert_eq!(mtt.tables.len(), 2);
    assert_eq!(mtt.get_rank(2).unwrap().chips, 0);
    assert_eq!(mtt.get_rank(1).unwrap().chips, 20000);
    assert_eq!(mtt.tables.get(&1), None);
    assert_eq!(mtt.tables.get(&2).unwrap().players.len(), 3);
    assert_eq!(mtt.tables.get(&3).unwrap().players.len(), 1);
    assert_eq!(
        effect.list_bridge_events().unwrap(),
        vec![
            (1, HoldemBridgeEvent::CloseTable),
            (
                3,
                HoldemBridgeEvent::SitinPlayers {
                    sitins: vec![MttTableSitin::new(1, 20000), MttTableSitin::new(3, 10000),],
                }
            )
        ]
    );
}

#[test]
fn test_game_result_given_2_tables_do_close_table() {
    // Create tables with number of players: 2, 2
    let mut mtt = helper::create_mtt_with_players(&[2, 2], 3);
    let mut effect = Effect::default();

    // Eliminate one player on table #1
    // Players distribution becomes 1, 2
    // The tables should be merged
    let game_result = HoldemBridgeEvent::GameResult {
        hand_id: 1,
        table_id: 1,
        player_results: vec![
            PlayerResult::new(
                2,
                0,
                Some(ChipsChange::Sub(10000)),
                PlayerResultStatus::Eliminated,
            ),
            PlayerResult::new(
                1,
                20000,
                Some(ChipsChange::Add(10000)),
                PlayerResultStatus::Normal,
            ),
        ],
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
        vec![
            (1, HoldemBridgeEvent::CloseTable),
            (
                2,
                HoldemBridgeEvent::SitinPlayers {
                    sitins: vec![MttTableSitin::new(1, 20000),]
                }
            )
        ]
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
        player_results: vec![],
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
        vec![
            (
                1,
                HoldemBridgeEvent::StartGame {
                    sitout_players: vec![1],
                    sb: DEFAULT_SB,
                    bb: DEFAULT_BB,
                }
            ),
            (
                2,
                HoldemBridgeEvent::SitinPlayers {
                    sitins: vec![MttTableSitin::new(1, 10000)]
                }
            )
        ]
    );
    assert_eq!(mtt.table_assigns.get(&1), None);
    assert_eq!(mtt.table_assigns_pending.get(&1), Some(&2));
    assert_eq!(mtt.table_assigns.get(&2), Some(&1));
    assert_eq!(mtt.table_assigns.get(&3), Some(&1));
    assert_eq!(mtt.table_assigns.get(&4), Some(&2));
    assert_eq!(mtt.tables.get(&1).map(|t| t.players.len()), Some(2));
    assert_eq!(mtt.tables.get(&2).map(|t| t.players.len()), Some(1));
}
