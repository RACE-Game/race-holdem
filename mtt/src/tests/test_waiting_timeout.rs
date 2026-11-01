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
fn init_state_schedules_waiting_timeout_until_start_time() {
    let data = MttAccountData {
        start_time: 1500,
        entry_close_time: 2000,
        ticket: 100,
        table_size: 9,
        start_chips: 5000,
        early_bird_chips: 0,
        blind_info: BlindInfo::default(),
        prize_rules: vec![PrizeRule {
            min_players: 0,
            prizes: vec![100],
        }],
        min_players: 2,
        rake: 0,
        bounty: 0,
        theme: None,
        subgame_bundle: "holdem".to_string(),
        check_in_duration: 0,
        max_afk_hands: 0,
    };
    let init_account = InitAccount {
        max_players: 100,
        data: borsh::to_vec(&data).unwrap(),
    };

    let mut effect = Effect::default();
    effect.timestamp = 500;
    Mtt::init_state(&mut effect, init_account).unwrap();

    assert_eq!(effect.wait_timeout, Some(1000));
    assert!(!effect.start_game);
}

#[test]
fn game_start_schedules_immediate_reconciliation_timeout() {
    let mut mtt = playing_mtt(9);
    mtt.stage = MttStage::Init;
    mtt.min_players = 2;
    add_player(&mut mtt, 1, 1000);
    add_player(&mut mtt, 2, 1000);
    mtt.update_alives();

    let mut effect = Effect::default();
    mtt.handle_event(&mut effect, Event::GameStart).unwrap();

    assert_eq!(effect.wait_timeout, Some(0));
}

#[test]
fn join_during_playing_schedules_reconciliation_timeout() {
    let mut mtt = playing_mtt(9);

    let mut effect = Effect::default();
    mtt.handle_event(
        &mut effect,
        Event::Join {
            players: vec![GamePlayer::new(1)],
        },
    )
    .unwrap();

    assert_eq!(effect.wait_timeout, Some(TABLE_READY_POLL_INTERVAL));
    assert!(effect.launch_sub_games.is_empty());
}

#[test]
fn deposit_during_playing_schedules_reconciliation_timeout() {
    let mut mtt = playing_mtt(9);
    add_player(&mut mtt, 1, 0);

    let mut effect = Effect::default();
    mtt.handle_event(
        &mut effect,
        Event::Deposit {
            deposits: vec![GameDeposit::new(1, 1000, 1)],
        },
    )
    .unwrap();

    assert_eq!(effect.wait_timeout, Some(TABLE_READY_POLL_INTERVAL));
    assert!(effect.launch_sub_games.is_empty());
}

#[test]
fn waiting_timeout_reschedules_while_initial_tables_are_pending() {
    let mut mtt = playing_mtt(9);
    for player_id in 1..=10 {
        add_player(&mut mtt, player_id, 1000);
    }
    mtt.update_alives();

    let mut effect = Effect::default();
    mtt.handle_event(&mut effect, Event::WaitingTimeout).unwrap();

    assert_eq!(effect.wait_timeout, Some(TABLE_READY_POLL_INTERVAL));
    assert_eq!(effect.launch_sub_games.len(), 2);
}

#[test]
fn waiting_timeout_does_not_reschedule_when_nothing_to_do() {
    let mut mtt = playing_mtt(9);
    add_player(&mut mtt, 1, 1000);
    add_player(&mut mtt, 2, 1000);
    add_ready_table(&mut mtt, 1, &[1, 2]);
    mtt.update_alives();

    let mut effect = Effect::default();
    mtt.handle_event(&mut effect, Event::WaitingTimeout).unwrap();

    assert_eq!(effect.wait_timeout, None);
    assert!(effect.launch_sub_games.is_empty());
}

#[test]
fn prize_distribution_schedules_bonus_timeout() {
    let mut mtt = playing_mtt(9);
    add_player(&mut mtt, 1, 1000);
    mtt.update_alives();
    mtt.total_prize = 1000;
    mtt.player_balances.insert(0, 1000);

    let mut effect = Effect::default();
    mtt.apply_prizes(&mut effect).unwrap();

    assert_eq!(effect.wait_timeout, Some(BONUS_DISTRIBUTION_DELAY));
}
