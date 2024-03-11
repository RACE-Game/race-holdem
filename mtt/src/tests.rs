//! Tests for Mtt.
mod helper;

use super::*;
use helper::*;
use race_holdem_base::essential::Player;
use race_test::prelude::*;

#[test]
fn test_init_state_with_new_game() -> anyhow::Result<()> {
    let acc = TestGameAccountBuilder::default()
        .with_deposit_range(1000, 1000)
        .with_data(MttAccountData {
            start_time: 1000,
            ticket: 1000,
            table_size: 6,
            blind_info: BlindInfo::default(),
            prize_rules: vec![50, 30, 20],
            theme: None,
        })
        .build();
    let init_account = acc.derive_init_account();
    let mut effect = Effect::default();
    let mtt = Mtt::init_state(&mut effect, init_account)?;
    assert_eq!(mtt.start_time, 1000);
    assert_eq!(mtt.alives, 0);
    assert_eq!(mtt.stage, MttStage::Init);
    assert!(mtt.table_assigns.is_empty());
    assert!(mtt.ranks.is_empty());
    assert!(mtt.tables.is_empty());
    assert_eq!(mtt.table_size, 6);
    assert_eq!(mtt.time_elapsed, 0);
    assert_eq!(mtt.timestamp, effect.timestamp);
    assert_eq!(mtt.blind_info, BlindInfo::default());
    Ok(())
}

#[test]
fn test_init_state_with_checkpoint() -> anyhow::Result<()> {
    let mut alice = TestClient::player("alice");
    let mut bob = TestClient::player("bob");
    let mut carol = TestClient::player("carol");
    let mut dave = TestClient::player("dave");
    let players = [&mut alice, &mut bob, &mut carol, &mut dave];
    let mtt = init_test_state(players)?;
    assert_eq!(mtt.start_time, 1001);
    assert_eq!(mtt.alives, 3);
    assert_eq!(mtt.stage, MttStage::Playing);
    assert_eq!(mtt.table_assigns.len(), 4);
    assert_eq!(mtt.ranks.len(), 4);
    assert_eq!(mtt.tables.len(), 2);
    assert_eq!(mtt.table_size, 2);
    assert_eq!(mtt.time_elapsed, 50);
    assert_eq!(mtt.timestamp, Effect::default().timestamp);
    assert_eq!(mtt.blind_info, BlindInfo::default());
    Ok(())
}

#[test]
fn test_create_tables() -> anyhow::Result<()> {
    let mut alice = TestClient::player("alice");
    let mut bob = TestClient::player("bob");
    let mut carol = TestClient::player("carol");
    let mut dave = TestClient::player("dave");
    let players = [&mut alice, &mut bob, &mut carol, &mut dave];
    let mut mtt = init_test_state(players)?;
    let evt = Event::GameStart;
    let mut effect = Effect::default();
    mtt.handle_event(&mut effect, evt)?;
    assert_eq!(
        mtt.table_assigns,
        BTreeMap::from([
            (alice.id(), 1),
            (bob.id(), 2),
            (carol.id(), 1),
            (dave.id(), 2),
        ])
    );
    println!("table_assigns {:?}", mtt.table_assigns);
    assert_eq!(
        effect.launch_sub_games,
        vec![
            SubGame::try_new(
                1,
                SUBGAME_BUNDLE_ADDR.into(),
                InitTableData {
                    table_id: 1,
                    table_size: 2,
                }
            )?,
            SubGame::try_new(
                2,
                SUBGAME_BUNDLE_ADDR.into(),
                InitTableData {
                    table_id: 2,
                    table_size: 2,
                }
            )?,
        ]
    );

    Ok(())
}

#[test]
fn test_close_table() -> anyhow::Result<()> {
    let mut pa = TestClient::player("pa");
    let mut pb = TestClient::player("pb");
    let mut pc = TestClient::player("pc");
    let mut pd = TestClient::player("pd");
    let mut pe = TestClient::player("pe");
    let mut pf = TestClient::player("pf");
    let mut pg = TestClient::player("pg");
    let mut ph = TestClient::player("ph");
    let mut pi = TestClient::player("pi");
    let mut pj = TestClient::player("pj");
    let mut pk = TestClient::player("pk");
    let mut pl = TestClient::player("pl");
    let players = [
        &mut pa, &mut pb, &mut pc, &mut pd, &mut pe, &mut pf, &mut pg, &mut ph, &mut pi, &mut pj,
        &mut pk, &mut pl,
    ];

    let mut mtt = setup_mtt_state(players)?;
    let evt = Event::GameStart { access_version: 0 };
    let mut effect = Effect::default();
    mtt.handle_event(&mut effect, evt)?;

    assert_eq!(mtt.alives, 12);
    assert_eq!(mtt.stage, MttStage::Playing);
    assert_eq!(mtt.tables.len(), 4);

    // T4 settles: 1 out and 2 alive so one emtpy seats available
    // T3 settles: 2 out and 1 alive, so T3 should be closed and the alive to be moved
    let t3_game_result = HoldemBridgeEvent::GameResult {
        table_id: 3,
        settles: vec![
            Settle::add(pc.id(), 2000),
            Settle::sub(pg.id(), 1000),
            Settle::sub(pk.id(), 1000),
        ],
        // checkpoint contains alive players only
        checkpoint: MttTableCheckpoint {
            btn: 0,
            players: vec![MttTablePlayer {
                id: pc.id(),
                chips: 3000,
                table_position: 0,
            }],
        },
    };

    let t4_game_result = HoldemBridgeEvent::GameResult {
        table_id: 4,
        settles: vec![
            Settle::add(pd.id(), 500),
            Settle::add(ph.id(), 500),
            Settle::sub(pl.id(), 1000),
        ],
        checkpoint: MttTableCheckpoint {
            btn: 0,
            players: vec![
                MttTablePlayer {
                    id: pd.id(),
                    chips: 1500,
                    table_position: 0,
                },
                MttTablePlayer {
                    id: ph.id(),
                    chips: 1500,
                    table_position: 1,
                },
            ],
        },
    };
    let evts = [
        Event::Bridge {
            dest: 4,
            raw: t4_game_result.try_to_vec()?,
        },
        Event::Bridge {
            dest: 3,
            raw: t3_game_result.try_to_vec()?,
        },
    ];

    for evt in evts {
        mtt.handle_event(&mut effect, evt)?;
    }

    assert_eq!(
        effect.bridge_events,
        vec![
            EmitBridgeEvent::try_new(
                4,
                HoldemBridgeEvent::Relocate {
                    players: vec![MttTablePlayer {
                        id: pc.id(),
                        chips: 3000,
                        table_position: 2
                    }]
                }
            )?,
            EmitBridgeEvent::try_new(3, HoldemBridgeEvent::CloseTable)?,
        ]
    );

    assert_eq!(mtt.tables.len(), 3); // one table gets closed
    assert_eq!(mtt.stage, MttStage::Playing);
    assert_eq!(mtt.alives, 9);
    Ok(())
}

#[test]
fn test_move_players() -> anyhow::Result<()> {
    let mut pa = TestClient::player("pa");
    let mut pb = TestClient::player("pb");
    let mut pc = TestClient::player("pc");
    let mut pd = TestClient::player("pd");
    let mut pe = TestClient::player("pe");
    let mut pf = TestClient::player("pf");
    let mut pg = TestClient::player("pg");
    let mut ph = TestClient::player("ph");
    let mut pi = TestClient::player("pi");
    let mut pj = TestClient::player("pj");
    let mut pk = TestClient::player("pk");
    let mut pl = TestClient::player("pl");
    let players = [
        &mut pa, &mut pb, &mut pc, &mut pd, &mut pe, &mut pf, &mut pg, &mut ph, &mut pi, &mut pj,
        &mut pk, &mut pl,
    ];
    let mut mtt = setup_mtt_state(players)?;
    let evt = Event::GameStart { access_version: 0 };
    let mut effect = Effect::default();
    effect.timestamp = 1001;
    mtt.handle_event(&mut effect, evt)?;

    assert_eq!(
        mtt.table_assigns,
        BTreeMap::from([
            (pa.id(), 1),
            (pb.id(), 2),
            (pc.id(), 3),
            (pd.id(), 4),
            (pe.id(), 1),
            (pf.id(), 2),
            (pg.id(), 3),
            (ph.id(), 4),
            (pi.id(), 1),
            (pj.id(), 2),
            (pk.id(), 3),
            (pl.id(), 4),
        ])
    );

    assert_eq!(
        effect.launch_sub_games,
        vec![
            SubGame::try_new(
                1,
                SUBGAME_BUNDLE_ADDR.into(),
                InitTableData {
                    btn: 0,
                    table_id: 1,
                    sb: 10,
                    bb: 20,
                    table_size: 3,
                    player_lookup: BTreeMap::from([
                        (pa.id(), Player::new(pa.id(), 1000, 0, 0)),
                        (pe.id(), Player::new(pe.id(), 1000, 1, 0)),
                        (pi.id(), Player::new(pi.id(), 1000, 2, 0)),
                    ])
                }
            )?,
            SubGame::try_new(
                2,
                SUBGAME_BUNDLE_ADDR.into(),
                InitTableData {
                    btn: 0,
                    table_id: 2,
                    sb: 10,
                    bb: 20,
                    table_size: 3,
                    player_lookup: BTreeMap::from([
                        (pb.id(), Player::new(pb.id(), 1000, 0, 0)),
                        (pf.id(), Player::new(pf.id(), 1000, 1, 0)),
                        (pj.id(), Player::new(pj.id(), 1000, 2, 0)),
                    ])
                }
            )?,
            SubGame::try_new(
                3,
                SUBGAME_BUNDLE_ADDR.into(),
                InitTableData {
                    btn: 0,
                    table_id: 3,
                    sb: 10,
                    bb: 20,
                    table_size: 3,
                    player_lookup: BTreeMap::from([
                        (pc.id(), Player::new(pc.id(), 1000, 0, 0)),
                        (pg.id(), Player::new(pg.id(), 1000, 1, 0)),
                        (pk.id(), Player::new(pk.id(), 1000, 2, 0)),
                    ])
                }
            )?,
            LaunchSubGame::try_new(
                4,
                SUBGAME_BUNDLE_ADDR.into(),
                InitTableData {
                    btn: 0,
                    table_id: 4,
                    sb: 10,
                    bb: 20,
                    table_size: 3,
                    player_lookup: BTreeMap::from([
                        (pd.id(), Player::new(pd.id(), 1000, 0, 0)),
                        (ph.id(), Player::new(ph.id(), 1000, 1, 0)),
                        (pl.id(), Player::new(pl.id(), 1000, 2, 0)),
                    ])
                }
            )?,
        ]
    );

    assert_eq!(mtt.start_time, 1001);
    assert_eq!(mtt.alives, 12);
    assert_eq!(mtt.stage, MttStage::Playing);
    assert_eq!(mtt.ranks.len(), 12);
    assert_eq!(mtt.tables.len(), 4);
    assert_eq!(mtt.table_size, 3);
    assert_eq!(mtt.time_elapsed, 1051);
    assert_eq!(mtt.timestamp, effect.timestamp);
    assert_eq!(mtt.blind_info, BlindInfo::default());

    // T3 settles: 2 players out, 1 player alive
    // T1 settles: 1 player will be moved to t3 (smallest)
    let t3_game_result = HoldemBridgeEvent::GameResult {
        table_id: 3,
        settles: vec![
            Settle::add(pc.id(), 2000),
            Settle::sub(pg.id(), 1000),
            Settle::sub(pk.id(), 1000),
        ],
        // checkpoint contains alive players only
        checkpoint: MttTableCheckpoint {
            btn: 0,
            players: vec![MttTablePlayer {
                id: pc.id(),
                chips: 3000,
                table_position: 0,
            }],
        },
    };

    let t1_game_result = HoldemBridgeEvent::GameResult {
        table_id: 1,
        settles: vec![
            Settle::add(pa.id(), 200),
            Settle::sub(pe.id(), 100),
            Settle::sub(pi.id(), 100),
        ],
        checkpoint: MttTableCheckpoint {
            btn: 0,
            players: vec![
                MttTablePlayer {
                    id: pa.id(),
                    chips: 1200,
                    table_position: 0,
                },
                MttTablePlayer {
                    id: pe.id(),
                    chips: 900,
                    table_position: 1,
                },
                MttTablePlayer {
                    id: pi.id(),
                    chips: 900,
                    table_position: 2,
                },
            ],
        },
    };

    let evts = [
        Event::Bridge {
            dest: 3,
            raw: t3_game_result.try_to_vec()?,
        },
        Event::Bridge {
            dest: 3,
            raw: t1_game_result.try_to_vec()?,
        },
    ];

    for evt in evts {
        mtt.handle_event(&mut effect, evt)?;
    }

    assert_eq!(
        effect.bridge_events,
        vec![
            EmitBridgeEvent::try_new(
                3,
                HoldemBridgeEvent::Relocate {
                    players: vec![MttTablePlayer {
                        id: pa.id(),
                        chips: 1200,
                        table_position: 1
                    }],
                }
            )?,
            EmitBridgeEvent::try_new(
                1,
                HoldemBridgeEvent::StartGame {
                    sb: 10,
                    bb: 20,
                    moved_players: vec![pa.id()] // player pa(id) left
                }
            )?
        ]
    );

    assert_eq!(mtt.tables.len(), 4); // no table gets closed
    assert_eq!(mtt.alives, 10);
    assert_eq!(mtt.stage, MttStage::Playing);

    Ok(())
}

#[test]
fn test_final_settle() -> anyhow::Result<()> {
    let mut alice = TestClient::player("alice");
    let mut bob = TestClient::player("bob");
    let mut carol = TestClient::player("carol");
    let mut dave = TestClient::player("dave");
    let players = [&mut alice, &mut bob, &mut carol, &mut dave];
    let mut mtt = helper::init_state_with_huge_amt(players)?;

    let evt = Event::GameStart { access_version: 0 };
    let mut effect = Effect::default();
    mtt.handle_event(&mut effect, evt)?;

    assert_eq!(mtt.tables.len(), 2);
    assert_eq!(mtt.ticket, 1_000_000_000);
    assert_eq!(mtt.total_prize, 4_000_000_000);

    // T1 settles: 1 player out and T1 gets closed
    // T2 settles: 1 player out and first three split the prize
    let t1_game_result = HoldemBridgeEvent::GameResult {
        table_id: 1,
        settles: vec![
            Settle::add(alice.id(), 2_000_000_000),
            Settle::sub(carol.id(), 2_000_000_000),
        ],
        checkpoint: MttTableCheckpoint {
            btn: 0,
            players: vec![MttTablePlayer {
                id: alice.id(),
                chips: 3_000_000_000,
                table_position: 1,
            }],
        },
    };

    let t2_game_result = HoldemBridgeEvent::GameResult {
        table_id: 2,
        settles: vec![
            Settle::add(alice.id(), 1_000_000_000),
            Settle::sub(bob.id(), 1_000_000_000),
        ],
        checkpoint: MttTableCheckpoint {
            btn: 0,
            players: vec![MttTablePlayer {
                id: alice.id(),
                chips: 4_000_000_000,
                table_position: 1,
            }],
        },
    };

    let evts = [
        Event::Bridge {
            dest: 2,
            raw: t1_game_result.try_to_vec()?,
        },
        Event::Bridge {
            dest: 2,
            raw: t2_game_result.try_to_vec()?,
        },
    ];

    for evt in evts {
        mtt.handle_event(&mut effect, evt)?;
    }
    assert!(mtt.has_winner());
    assert_eq!(mtt.alives, 1);
    assert_eq!(mtt.tables.len(), 1); // one last table
    assert_eq!(
        effect.settles,
        vec![
            Settle::add(alice.id(), (4_000_000_000 * 50 / 100) - 1_000_000_000),
            Settle::add(bob.id(), (4_000_000_000 * 30 / 100) - 1_000_000_000),
            Settle::sub(carol.id(), 1_000_000_000 - (4_000_000_000 * 20 / 100)),
            Settle::sub(dave.id(), 1_000_000_000),
        ]
    );
    assert_eq!(mtt.stage, MttStage::Completed); // mtt should be completed
    Ok(())
}

#[test]
fn test_no_enough_players_complete() -> anyhow::Result<()> {
    let acc = TestGameAccountBuilder::default()
        .with_deposit_range(1000, 1000)
        .with_data(MttAccountData {
            start_time: 1001,
            table_size: 6,
            blind_info: BlindInfo::default(),
            prize_rules: vec![50, 30, 20],
            theme: None,
        })
        .build();
    let init_account = acc.derive_init_account();
    let mut effect = Effect::default();
    effect.timestamp = 1000;
    let mut mtt = Mtt::init_state(&mut effect, init_account)?;

    assert_eq!(mtt.stage, MttStage::Init);

    mtt.handle_event(&mut effect, Event::Ready)?;
    mtt.handle_event(&mut effect, Event::WaitingTimeout)?;

    assert_eq!(mtt.stage, MttStage::Completed);
    Ok(())
}
