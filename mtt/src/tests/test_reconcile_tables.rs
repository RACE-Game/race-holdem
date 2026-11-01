use super::*;

fn playing_mtt(table_size: u8) -> Mtt {
    Mtt {
        stage: MttStage::Playing,
        table_size,
        start_chips: 1000,
        entry_close_time: 1_000_000,
        blind_info: BlindInfo::default(),
        prize_rules: vec![PrizeRule {
            min_players: 0,
            prizes: vec![100],
        }],
        ..Default::default()
    }
}

fn add_player(mtt: &mut Mtt, id: u64, chips: u64) {
    let pos = mtt.ranks.len() as u16;
    mtt.ranks.push(PlayerRank::new(
        id,
        chips,
        PlayerRankStatus::Pending,
        pos,
        vec![chips],
        DEFAULT_TIME_CARDS,
    ));
}

fn add_ready_table(mtt: &mut Mtt, table_id: usize, player_ids: &[u64]) {
    let players = player_ids
        .iter()
        .enumerate()
        .map(|(idx, player_id)| {
            mtt.table_assigns.insert(*player_id, table_id);
            MttTablePlayer::new(*player_id, 1000, idx as u8, DEFAULT_TIME_CARDS)
        })
        .collect();

    mtt.tables.insert(
        table_id,
        MttTableState::new(table_id, DEFAULT_SB, DEFAULT_BB, DEFAULT_ANTE, players),
    );
}

#[test]
fn initial_waiting_timeout_launches_all_initial_tables() {
    let mut mtt = playing_mtt(9);
    mtt.stage = MttStage::Init;
    mtt.min_players = 2;
    for player_id in 1..=10 {
        add_player(&mut mtt, player_id, 1000);
    }

    let mut effect = Effect::default();
    mtt.handle_event(&mut effect, Event::GameStart).unwrap();

    assert_eq!(mtt.stage, MttStage::Playing);
    assert!(effect.launch_sub_games.is_empty());
    assert_eq!(effect.wait_timeout, Some(0));

    let mut effect = Effect::default();
    mtt.handle_event(&mut effect, Event::WaitingTimeout).unwrap();

    assert_eq!(effect.launch_sub_games.len(), 2);
    assert_eq!(mtt.launched_table_ids.len(), 2);
    assert_eq!(mtt.table_assigns.len(), 10);
    assert_eq!(effect.wait_timeout, Some(TABLE_READY_POLL_INTERVAL));
}

#[test]
fn resume_with_partial_tables_launches_missing_tables() {
    let mut mtt = playing_mtt(9);
    for player_id in 1..=10 {
        add_player(&mut mtt, player_id, 1000);
    }

    let ready_player_ids: Vec<u64> = (1..=8).collect();
    add_ready_table(&mut mtt, 1, &ready_player_ids);

    let mut effect = Effect::default();
    mtt.handle_event(&mut effect, Event::WaitingTimeout).unwrap();

    assert_eq!(effect.launch_sub_games.len(), 1);
    assert_eq!(mtt.launched_table_ids.len(), 1);
    assert_eq!(effect.wait_timeout, Some(TABLE_READY_POLL_INTERVAL));
}

#[test]
fn join_during_initial_table_creation_does_not_directly_seat_or_launch() {
    let mut mtt = playing_mtt(9);
    for player_id in 1..=8 {
        add_player(&mut mtt, player_id, 1000);
    }
    mtt.update_alives();

    let mut effect = Effect::default();
    mtt.handle_event(&mut effect, Event::WaitingTimeout).unwrap();
    assert_eq!(effect.launch_sub_games.len(), 1);

    let table_id = mtt.launched_table_ids[0];
    let table_init = MttTableInit::new(
        table_id,
        DEFAULT_SB,
        DEFAULT_BB,
        DEFAULT_ANTE,
        (1..=8)
            .map(|player_id| MttTablePlayerInit::new(player_id, 1000))
            .collect(),
        0,
    );

    let mut join_effect = Effect::default();
    mtt.handle_event(
        &mut join_effect,
        Event::Join {
            players: vec![GamePlayer::new(9)],
        },
    )
    .unwrap();

    assert!(join_effect.launch_sub_games.is_empty());
    assert!(join_effect.bridge_events.is_empty());
    assert_eq!(join_effect.wait_timeout, Some(TABLE_READY_POLL_INTERVAL));

    let mut deposit_effect = Effect::default();
    mtt.handle_event(
        &mut deposit_effect,
        Event::Deposit {
            deposits: vec![GameDeposit::new(9, 1000, 1)],
        },
    )
    .unwrap();

    assert!(deposit_effect.launch_sub_games.is_empty());
    assert!(deposit_effect.bridge_events.is_empty());
    assert_eq!(deposit_effect.wait_timeout, Some(TABLE_READY_POLL_INTERVAL));

    let mut ready_effect = Effect::default();
    mtt.handle_event(
        &mut ready_effect,
        Event::SubGameReady {
            game_id: table_id,
            max_players: mtt.table_size as u16,
            init_data: borsh::to_vec(&table_init).unwrap(),
        },
    )
    .unwrap();

    assert!(ready_effect.launch_sub_games.is_empty());

    let mut timeout_effect = Effect::default();
    mtt.handle_event(&mut timeout_effect, Event::WaitingTimeout).unwrap();

    assert!(timeout_effect.launch_sub_games.is_empty());
    assert_eq!(
        timeout_effect
            .list_bridge_events::<HoldemBridgeEvent>()
            .unwrap(),
        vec![(
            table_id,
            HoldemBridgeEvent::SitinPlayers {
                sitins: vec![MttTableSitin::new(9, 1000, DEFAULT_TIME_CARDS, true)]
            }
        )]
    );
    assert_eq!(timeout_effect.wait_timeout, None);
}
