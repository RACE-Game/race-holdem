use super::*;

fn add_player_to_mtt(mtt: &mut Mtt) -> u64 {
    let l = mtt.ranks.len();
    let player_id = (l + 1) as u64;
    mtt.ranks.push(PlayerRank::new(
        player_id,
        1000,
        PlayerRankStatus::Pending,
        l as u16,
        vec![1000],
    ));
    return player_id;
}

#[test]
fn sit_players_given_all_tables_full_do_create_new_table() {
    let mut mtt = helper::create_mtt_with_players(&[3, 3, 3], 3);
    mtt.stage = MttStage::Playing;
    let mut effect = Effect::default();

    let pid = add_player_to_mtt(&mut mtt);
    mtt.sit_players(&mut effect, vec![pid]).unwrap();

    effect.print_logs();

    assert_eq!(effect.launch_sub_games.len(), 1);
}

#[test]
fn sit_players_given_two_table_with_one_player_do_sit_to_non_empty_table() {
    let mut mtt = helper::create_mtt_with_players(&[1, 0], 3);
    mtt.stage = MttStage::Playing;
    let mut effect = Effect::default();

    let pid = add_player_to_mtt(&mut mtt);
    mtt.sit_players(&mut effect, vec![pid]).unwrap();

    effect.print_logs();

    assert_eq!(
        effect.list_bridge_events().unwrap(),
        vec![(
            1,
            HoldemBridgeEvent::SitinPlayers {
                sitins: vec![MttTableSitin::new(pid, 1000)]
            }
        )]
    );
}

#[test]
fn sit_players_given_two_table_with_one_player_do_sit_to_non_empty_table_2() {
    let mut mtt = helper::create_mtt_with_players(&[0, 1], 3);
    mtt.stage = MttStage::Playing;
    let mut effect = Effect::default();

    let pid = add_player_to_mtt(&mut mtt);
    mtt.sit_players(&mut effect, vec![pid]).unwrap();

    effect.print_logs();

    assert_eq!(
        effect.list_bridge_events().unwrap(),
        vec![(
            2,
            HoldemBridgeEvent::SitinPlayers {
                sitins: vec![MttTableSitin::new(pid, 1000)]
            }
        )]
    );
}

#[test]
fn sit_multiple_players_when_all_tables_are_full_creates_new_tables() {
    let mut mtt = helper::create_mtt_with_players(&[3, 3, 3], 3);
    mtt.stage = MttStage::Playing;
    let mut effect = Effect::default();

    let pid1 = add_player_to_mtt(&mut mtt);
    let pid2 = add_player_to_mtt(&mut mtt);
    mtt.sit_players(&mut effect, vec![pid1, pid2]).unwrap();

    effect.print_logs();

    let (sb, bb) = mtt.calc_blinds().unwrap();
    let table_created = MttTableState::new(0, sb, bb, vec![MttTablePlayer::new(10, 1000, 0), MttTablePlayer::new(11, 1000, 1)]);
    assert_eq!(
        effect.launch_sub_games,
        vec![
            SubGame::try_new(0, mtt.subgame_bundle.clone(), mtt.table_size as _, &table_created).unwrap(),
        ]
    ); // Expect one new table to be created
}

#[test]
fn sit_multiple_players_with_empty_tables_should_fill_sparse_tables() {
    let mut mtt = helper::create_mtt_with_players(&[2, 1], 3);
    mtt.stage = MttStage::Playing;
    let mut effect = Effect::default();

    let pid1 = add_player_to_mtt(&mut mtt);
    let pid2 = add_player_to_mtt(&mut mtt);
    mtt.sit_players(&mut effect, vec![pid1, pid2]).unwrap();

    effect.print_logs();

    assert_eq!(
        effect.list_bridge_events::<HoldemBridgeEvent>().unwrap(),
        vec![
            (
                1,
                HoldemBridgeEvent::SitinPlayers {
                    sitins: vec![MttTableSitin::new(pid2, 1000)]
                }
            ),
            (
                2,
                HoldemBridgeEvent::SitinPlayers {
                    sitins: vec![MttTableSitin::new(pid1, 1000)]
                }
            )
        ]
    );
}

#[test]
fn sit_players_in_non_playing_stage_no_ops() {
    let mut mtt = helper::create_mtt_with_players(&[2, 1], 3);
    mtt.stage = MttStage::Init;
    let mut effect = Effect::default();

    let pid = add_player_to_mtt(&mut mtt);
    mtt.sit_players(&mut effect, vec![pid]).unwrap();

    assert!(effect
        .list_bridge_events::<HoldemBridgeEvent>()
        .unwrap()
        .is_empty());
    assert!(effect.launch_sub_games.is_empty());
}

#[test]
fn sit_many_players_creating_multiple_tables() {
    let mut mtt = create_mtt_with_players(&[], 6);
    mtt.stage = MttStage::Playing;
    let mut effect = Effect::default();

    for _ in 0..20 {
        add_player_to_mtt(&mut mtt);
    }
    let player_ids: Vec<_> = mtt.ranks.iter().map(|r| r.id).collect();
    mtt.sit_players(&mut effect, player_ids).unwrap();

    effect.print_logs();

    assert_eq!(effect.launch_sub_games.len(), 4); // Expecting 4 tables to create
}

#[test]
fn sit_players_in_existing_and_new_tables() {
    let mut mtt = create_mtt_with_players(&[2, 4], 6);
    mtt.stage = MttStage::Playing;
    let mut effect = Effect::default();

    // Add new players
    let new_players: Vec<u64> = (0..10).map(|_| add_player_to_mtt(&mut mtt)).collect();

    // Sit new players
    mtt.sit_players(&mut effect, new_players).unwrap();

    effect.print_logs();

    let (sb, bb) = mtt.calc_blinds().unwrap();
    let table_created = MttTableState::new(0, sb, bb, vec![
        MttTablePlayer::new(13, 1000, 0),
        MttTablePlayer::new(14, 1000, 1),
        MttTablePlayer::new(15, 1000, 2),
        MttTablePlayer::new(16, 1000, 3),
    ]);

    assert_eq!(effect.launch_sub_games, vec![
        SubGame::try_new(0, mtt.subgame_bundle.clone(), mtt.table_size as _, &table_created).unwrap()
    ]);

    assert_eq!(effect.list_bridge_events().unwrap(), vec![
        (1, HoldemBridgeEvent::SitinPlayers {
            sitins: vec![
                MttTableSitin::new(7, 1000),
                MttTableSitin::new(8, 1000),
                MttTableSitin::new(9, 1000),
                MttTableSitin::new(11, 1000),
            ]
        }),
        (2, HoldemBridgeEvent::SitinPlayers {
            sitins: vec![
                MttTableSitin::new(10, 1000),
                MttTableSitin::new(12, 1000),
            ]
        }),
    ]);
}
