//! Helper functions for testing Mtt crate
use super::*;

pub fn init_test_state(players: [&mut TestClient; 4]) -> anyhow::Result<Mtt> {
    let mut players_iter = players.into_iter();
    let alice = players_iter.next().unwrap();
    let bob = players_iter.next().unwrap();
    let carol = players_iter.next().unwrap();
    let dave = players_iter.next().unwrap();
    let acc = TestGameAccountBuilder::default()
        .with_data(MttAccountData {
            start_time: 1000,
            ticket: 1000,
            table_size: 2,
            blind_info: BlindInfo::default(),
            prize_rules: vec![50, 30, 20],
            theme: None,
        })
        .with_deposit_range(1000, 1000)
        .add_player(alice, 1000)
        .add_player(bob, 1000)
        .add_player(carol, 2000)
        .add_player(dave, 0)
        .with_checkpoint(MttCheckpoint {
            start_time: 1001,
            timestamp: 0,
            ranks: vec![
                PlayerRankCheckpoint {
                    id: alice.id(),
                    chips: 1000,
                    table_id: 1,
                },
                PlayerRankCheckpoint {
                    id: bob.id(),
                    chips: 1000,
                    table_id: 2,
                },
                PlayerRankCheckpoint {
                    id: carol.id(),
                    chips: 2000,
                    table_id: 1,
                },
                PlayerRankCheckpoint {
                    id: dave.id(),
                    chips: 0,
                    table_id: 2,
                },
            ],
            tables: BTreeMap::from([
                (
                    1,
                    MttTable {
                        hand_id: 0,
                        btn: 0,
                        sb: 10,
                        bb: 20,
                        players: vec![
                            MttTablePlayer {
                                id: alice.id(),
                                chips: 1000,
                                table_position: 0,
                            },
                            MttTablePlayer {
                                id: carol.id(),
                                chips: 2000,
                                table_position: 1,
                            },
                        ],
                        next_game_start: 0,
                    },
                ),
                (
                    2,
                    MttTable {
                        hand_id: 0,
                        btn: 0,
                        sb: 10,
                        bb: 20,
                        players: vec![MttTablePlayer {
                            id: bob.id(),
                            chips: 1000,
                            table_position: 0,
                        }],
                        next_game_start: 0,
                    },
                ),
            ]),
            time_elapsed: 50,
            total_prize: 4000,
            stage: MttStage::Playing,
        })
        .build();
    let init_account = acc.derive_init_account();
    let mut effect = Effect::default();
    Ok(Mtt::init_state(&mut effect, init_account)?)
}

// Like init_test_state but with larger table size and more players
pub fn setup_mtt_state(players: [&mut TestClient; 12]) -> anyhow::Result<Mtt> {
    let mut players_iter = players.into_iter();
    let pa = players_iter.next().unwrap();
    let pb = players_iter.next().unwrap();
    let pc = players_iter.next().unwrap();
    let pd = players_iter.next().unwrap();
    let pe = players_iter.next().unwrap();
    let pf = players_iter.next().unwrap();
    let pg = players_iter.next().unwrap();
    let ph = players_iter.next().unwrap();
    let pi = players_iter.next().unwrap();
    let pj = players_iter.next().unwrap();
    let pk = players_iter.next().unwrap();
    let pl = players_iter.next().unwrap();

    let acc = TestGameAccountBuilder::default()
        .with_max_players(20)
        .with_deposit_range(1000, 1000)
        .with_data(MttAccountData {
            start_time: 1000,
            ticket: 1000,
            table_size: 3,
            blind_info: BlindInfo::default(),
            prize_rules: vec![50, 30, 20],
            theme: None,
        })
        .add_player(pa, 1000)
        .add_player(pb, 1000)
        .add_player(pc, 1000)
        .add_player(pd, 1000)
        .add_player(pe, 1000)
        .add_player(pf, 1000)
        .add_player(pg, 1000)
        .add_player(ph, 1000)
        .add_player(pi, 1000)
        .add_player(pj, 1000)
        .add_player(pk, 1000)
        .add_player(pl, 1000)
        .with_checkpoint(MttCheckpoint {
            start_time: 1001,
            timestamp: 0,
            ranks: vec![
                PlayerRankCheckpoint {
                    // pa
                    id: pa.id(),
                    chips: 1000,
                    table_id: 1,
                },
                PlayerRankCheckpoint {
                    // pb
                    id: pb.id(),
                    chips: 1000,
                    table_id: 2,
                },
                PlayerRankCheckpoint {
                    // pc
                    id: pc.id(),
                    chips: 1000,
                    table_id: 3,
                },
                PlayerRankCheckpoint {
                    // pd
                    id: pd.id(),
                    chips: 1000,
                    table_id: 4,
                },
                PlayerRankCheckpoint {
                    // pe
                    id: pe.id(),
                    chips: 1000,
                    table_id: 1,
                },
                PlayerRankCheckpoint {
                    // pf
                    id: pf.id(),
                    chips: 1000,
                    table_id: 2,
                },
                PlayerRankCheckpoint {
                    // pg
                    id: pg.id(),
                    chips: 1000,
                    table_id: 3,
                },
                PlayerRankCheckpoint {
                    // ph
                    id: ph.id(),
                    chips: 1000,
                    table_id: 4,
                },
                PlayerRankCheckpoint {
                    // pi
                    id: pi.id(),
                    chips: 1000,
                    table_id: 1,
                },
                PlayerRankCheckpoint {
                    // pj
                    id: pj.id(),
                    chips: 1000,
                    table_id: 2,
                },
                PlayerRankCheckpoint {
                    // pk
                    id: pk.id(),
                    chips: 1000,
                    table_id: 3,
                },
                PlayerRankCheckpoint {
                    // pl
                    id: pl.id(),
                    chips: 1000,
                    table_id: 4,
                },
            ],
            tables: BTreeMap::from([
                (
                    1,
                    MttTable {
                        hand_id: 0,
                        next_game_start: 0,
                        btn: 0,
                        sb: 10,
                        bb: 20,
                        players: vec![
                            MttTablePlayer {
                                id: pa.id(),
                                chips: 1000,
                                table_position: 0,
                            },
                            MttTablePlayer {
                                id: pe.id(),
                                chips: 1000,
                                table_position: 1,
                            },
                            MttTablePlayer {
                                id: pi.id(),
                                chips: 1000,
                                table_position: 2,
                            },
                        ],
                    },
                ),
                (
                    2,
                    MttTable {
                        hand_id: 0,
                        next_game_start: 0,
                        btn: 0,
                        sb: 10,
                        bb: 20,
                        players: vec![
                            MttTablePlayer {
                                id: pb.id(),
                                chips: 1000,
                                table_position: 0,
                            },
                            MttTablePlayer {
                                id: pf.id(),
                                chips: 1000,
                                table_position: 1,
                            },
                            MttTablePlayer {
                                id: pj.id(),
                                chips: 1000,
                                table_position: 2,
                            },
                        ],
                    },
                ),
                (
                    3,
                    MttTable {
                        hand_id: 0,
                        next_game_start: 0,
                        btn: 0,
                        sb: 10,
                        bb: 20,
                        players: vec![
                            MttTablePlayer {
                                id: pc.id(),
                                chips: 1000,
                                table_position: 0,
                            },
                            MttTablePlayer {
                                id: pg.id(),
                                chips: 1000,
                                table_position: 1,
                            },
                            MttTablePlayer {
                                id: pk.id(),
                                chips: 1000,
                                table_position: 2,
                            },
                        ],
                    },
                ),
                (
                    4,
                    MttTable {
                        hand_id: 0,
                        next_game_start: 0,
                        btn: 0,
                        sb: 10,
                        bb: 20,
                        players: vec![
                            MttTablePlayer {
                                id: pd.id(),
                                chips: 1000,
                                table_position: 0,
                            },
                            MttTablePlayer {
                                id: ph.id(),
                                chips: 1000,
                                table_position: 1,
                            },
                            MttTablePlayer {
                                id: pl.id(),
                                chips: 1000,
                                table_position: 2,
                            },
                        ],
                    },
                ),
            ]),
            time_elapsed: 50,
            total_prize: 12_000,
            stage: MttStage::Playing,
        })
        .build();
    let init_account = acc.derive_init_account();
    let mut effect = Effect::default();
    Ok(Mtt::init_state(&mut effect, init_account)?)
}

pub fn init_state_with_huge_amt(players: [&mut TestClient; 4]) -> anyhow::Result<Mtt> {
    let mut players_iter = players.into_iter();
    let alice = players_iter.next().unwrap();
    let bob = players_iter.next().unwrap();
    let carol = players_iter.next().unwrap();
    let dave = players_iter.next().unwrap();
    let acc = TestGameAccountBuilder::default()
        .with_deposit_range(1_000_000_000, 1_000_000_000)
        .with_data(MttAccountData {
            start_time: 1000,
            ticket: 1_000_000_000,
            table_size: 2,
            blind_info: BlindInfo::default(),
            prize_rules: vec![50, 30, 20],
            theme: None,
        })
        .add_player(alice, 1_000_000_000)
        .add_player(bob, 1_000_000_000)
        .add_player(carol, 2_000_000_000)
        .add_player(dave, 0)
        .with_checkpoint(MttCheckpoint {
            start_time: 1001,
            timestamp: 0,
            ranks: vec![
                PlayerRankCheckpoint {
                    id: alice.id(),
                    chips: 1_000_000_000,
                    table_id: 1,
                },
                PlayerRankCheckpoint {
                    id: bob.id(),
                    chips: 1_000_000_000,
                    table_id: 2,
                },
                PlayerRankCheckpoint {
                    id: carol.id(),
                    chips: 2_000_000_000,
                    table_id: 1,
                },
                PlayerRankCheckpoint {
                    id: dave.id(),
                    chips: 0,
                    table_id: 2,
                },
            ],
            tables: BTreeMap::from([
                (
                    1,
                    MttTable {
                        hand_id: 0,
                        next_game_start: 0,
                        btn: 0,
                        sb: 10,
                        bb: 20,
                        players: vec![
                            MttTablePlayer {
                                id: alice.id(),
                                chips: 1_000_000_000,
                                table_position: 0,
                            },
                            MttTablePlayer {
                                id: carol.id(),
                                chips: 2_000_000_000,
                                table_position: 1,
                            },
                        ],
                    },
                ),
                (
                    2,
                    MttTable {
                        hand_id: 0,
                        next_game_start: 0,
                        btn: 0,
                        sb: 10,
                        bb: 20,
                        players: vec![MttTablePlayer {
                            id: bob.id(),
                            chips: 1_000_000_000,
                            table_position: 0,
                        }],
                    },
                ),
            ]),
            time_elapsed: 50,
            total_prize: 4_000_000_000,
            stage: MttStage::Playing,
        })
        .build();
    let init_account = acc.derive_init_account();
    let mut effect = Effect::default();
    Ok(Mtt::init_state(&mut effect, init_account)?)
}
