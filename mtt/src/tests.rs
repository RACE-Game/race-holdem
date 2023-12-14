use std::collections::HashMap;

use race_api::prelude::*;
use race_holdem_base::essential::WAIT_TIMEOUT_DEFAULT;
use race_test::prelude::*;

use super::*;

#[test]
pub fn test_props_serialize() {
    let props = MttProperties {
        start_time: 0,
        table_size: 3,
        blind_base: 10,
        blind_interval: 60_000,
        blind_rules: vec![],
    };
    println!("{:?}", props.try_to_vec());
}

#[test]
pub fn test_new_player_join() -> anyhow::Result<()> {
    let mut effect = Effect::default();
    let mut handler = Mtt::default();
    assert_eq!(handler.ranks.len(), 0);
    let event = sync_new_players(&[("alice", 0, 1000)], 1);
    handler.handle_event(&mut effect, event)?;
    assert_eq!(handler.ranks.len(), 1);
    Ok(())
}

#[test]
pub fn test_game_start() -> anyhow::Result<()> {
    let mut effect = Effect::default();
    let mut handler = Mtt::default();
    handler.blind_rules = default_blind_rules();
    handler.table_size = 2;
    // Add 5 players
    let event = sync_new_players(
        &[
            ("p1", 0, 1000),
            ("p2", 1, 1000),
            ("p3", 2, 1000),
            ("p4", 3, 1000),
            ("p5", 4, 1000),
        ],
        1,
    );
    handler.handle_event(&mut effect, event)?;
    let event = Event::GameStart { access_version: 1 };
    handler.handle_event(&mut effect, event)?;
    assert_eq!(handler.ranks.len(), 5);
    assert_eq!(handler.games.len(), 3);
    Ok(())
}

pub fn create_sync_event(
    ctx: &GameContext,
    players: &[&TestClient],
    transactor: &TestClient,
) -> Event {
    let av = ctx.get_access_version() + 1;
    let max_players = 9usize;
    let used_pos: Vec<usize> = ctx.get_players().iter().map(|p| p.position).collect();
    let mut new_players = Vec::new();
    for (i, p) in players.iter().enumerate() {
        let mut position = i;
        // If a position already taken, append the new player to the last
        if used_pos.get(position).is_some() {
            if position + 1 < max_players {
                position = ctx.count_players() as usize + 1;
            } else {
                println!("Game is full");
            }
        }
        new_players.push(PlayerJoin {
            addr: p.get_addr(),
            balance: 10_000,
            position: position as u16,
            access_version: av,
            verify_key: p.get_addr(),
        })
    }

    Event::Sync {
        new_players,
        new_servers: vec![],
        transactor_addr: transactor.get_addr(),
        access_version: av,
    }
}

#[test]
pub fn integration_simple_game_test() -> anyhow::Result<()> {
    let mtt_props = MttProperties {
        table_size: 2,
        blind_base: 50,
        blind_interval: 10000000,
        start_time: 0,
        ..Default::default()
    };
    let mut transactor = TestClient::transactor("Tx");
    let game_account = TestGameAccountBuilder::default()
        .with_data(mtt_props)
        .set_transactor(&transactor)
        .build();
    let mut ctx = GameContext::try_new(&game_account).unwrap();
    let mut handler = TestHandler::<Mtt>::init_state(&mut ctx, &game_account).unwrap();

    let alice = TestClient::player("Alice");
    let bob = TestClient::player("Bob");

    let sync_evt = create_sync_event(&ctx, &[&alice, &bob], &transactor);

    handler.handle_until_no_events(&mut ctx, &sync_evt, vec![&mut transactor])?;

    {
        let state = handler.get_state();
        assert_eq!(state.ranks.len(), 2);
    }

    let wait_timeout_evt = Event::WaitingTimeout;

    handler.handle_until_no_events(&mut ctx, &wait_timeout_evt, vec![&mut transactor])?;

    {
        let state = handler.get_state();
        assert_eq!(state.games.len(), 1);
        let (_, game0) = state.games.first_key_value().unwrap();
        assert_eq!(game0.player_map.len(), 2);
        assert_eq!(game0.stage, HoldemStage::Play);
        // Table assigns:
        // 1. Alice, Dave
        // 2. Bob, Eva
        // 3. Carol
    }

    let action_timeout_evt = Event::ActionTimeout {
        player_addr: "Bob".to_string(),
    };

    handler.handle_until_no_events(&mut ctx, &action_timeout_evt, vec![&mut transactor])?;

    {
        let state = handler.get_state();
        let (_, game0) = state.games.first_key_value().unwrap();
        assert_eq!(game0.player_map.len(), 2);
        assert_eq!(game0.stage, HoldemStage::Settle);
        assert_eq!(
            ctx.get_dispatch(),
            &Some(DispatchEvent::new(
                Event::WaitingTimeout,
                WAIT_TIMEOUT_DEFAULT
            ))
        );
    }

    ctx.set_timestamp(ctx.get_timestamp() + 5000);
    handler.handle_until_no_events(&mut ctx, &Event::WaitingTimeout, vec![&mut transactor])?;

    {
        let state = handler.get_state();
        let (_, game0) = state.games.first_key_value().unwrap();
        assert_eq!(game0.deck_random_id, 2);
    }

    Ok(())
}

#[test]
/// 5 players, three tables, test table merging
pub fn integration_table_merge_test() -> anyhow::Result<()> {
    let mtt_props = MttProperties {
        table_size: 2,
        blind_base: 50,
        blind_interval: 10000000,
        start_time: 0,
        ..Default::default()
    };

    let mut transactor = TestClient::transactor("Tx");
    let game_account = TestGameAccountBuilder::default()
        .with_data(mtt_props)
        .set_transactor(&transactor)
        .build();
    let mut ctx = GameContext::try_new(&game_account).unwrap();
    let mut handler = TestHandler::<Mtt>::init_state(&mut ctx, &game_account).unwrap();

    let alice = TestClient::player("Alice");
    let bob = TestClient::player("Bob");
    let carol = TestClient::player("Carol");
    let dave = TestClient::player("Dave");
    let eva = TestClient::player("Eva");

    let sync_evt = create_sync_event(&ctx, &[&alice, &bob, &carol, &dave, &eva], &transactor);

    handler.handle_until_no_events(&mut ctx, &sync_evt, vec![&mut transactor])?;

    {
        let state = handler.get_state();
        assert_eq!(state.ranks.len(), 5);
    }

    let wait_timeout_evt = Event::WaitingTimeout;

    handler.handle_until_no_events(&mut ctx, &wait_timeout_evt, vec![&mut transactor])?;

    {
        let state = handler.get_state();
        assert_eq!(state.games.len(), 3);
        let (_, game0) = state.games.first_key_value().unwrap();
        assert_eq!(game0.player_map.len(), 2);
        assert_eq!(game0.stage, HoldemStage::Play);
        // Table assigns:
        // 1. Alice, Dave
        // 2. Bob, Eva
        // 3. Carol
    }

    // Dave allin
    let dave_allin = dave.custom_event(GameEvent::Raise(10000));
    handler.handle_until_no_events(&mut ctx, &dave_allin, vec![&mut transactor])?;

    {
        let state = handler.get_state();
        let (_, game0) = state.games.first_key_value().unwrap();
        assert_eq!(game0.deck_random_id, 1);
        assert_eq!(game0.player_map.get("Alice").unwrap().chips, 9900); // -100 for blind
        assert_eq!(game0.player_map.get("Dave").unwrap().chips, 0);
        // TODO: Check the dispatching
    }

    let alice_call = alice.custom_event(GameEvent::Call);
    ctx.add_revealed_random(
        1,
        HashMap::from([
            (0, "sa".to_string()), // Alice
            (1, "sk".to_string()), // Alice
            (2, "ha".to_string()), // Dave
            (3, "hk".to_string()), // Dave
            (4, "sq".to_string()),
            (5, "sj".to_string()),
            (6, "st".to_string()),
            (7, "s8".to_string()),
            (8, "s9".to_string()),
        ]),
    )?;
    handler.handle_until_no_events(&mut ctx, &alice_call, vec![&mut transactor])?;

    {
        let state = handler.get_state();
        assert_eq!(state.games.len(), 2); // The original first game was closed
        assert_eq!(
            *state.ranks.first().unwrap(),
            PlayerRank::new("Alice", 20000, PlayerRankStatus::Alive)
        );
        println!("Ranks: {:?}", state.ranks);
        assert_eq!(
            *state.ranks.last().unwrap(),
            PlayerRank::new("Dave", 0, PlayerRankStatus::Out)
        );
    }

    ctx.add_revealed_random(
        2,
        HashMap::from([
            (0, "sa".to_string()), // Bob
            (1, "sk".to_string()), // Bob
            (2, "ha".to_string()), // Eva
            (3, "hk".to_string()), // Eva
            (4, "sq".to_string()),
            (5, "sj".to_string()),
            (6, "st".to_string()),
            (7, "s8".to_string()),
            (8, "s9".to_string()),
        ]),
    )?;

    let eva_allin = eva.custom_event(GameEvent::Raise(10000));
    handler.handle_until_no_events(&mut ctx, &eva_allin, vec![&mut transactor])?;
    let bob_call = bob.custom_event(GameEvent::Call);
    handler.handle_until_no_events(&mut ctx, &bob_call, vec![&mut transactor])?;

    {
        let state = handler.get_state();
        let (_, game0) = state.games.first_key_value().unwrap();
        assert_eq!(game0.player_map.len(), 1);
        assert_eq!(game0.stage, HoldemStage::Runner);
        assert_eq!(
            state.ranks,
            vec![
                PlayerRank::new("Alice", 20000, PlayerRankStatus::Alive),
                PlayerRank::new("Bob", 20000, PlayerRankStatus::Alive),
                PlayerRank::new("Carol", 10000, PlayerRankStatus::Alive),
                PlayerRank::new("Eva", 0, PlayerRankStatus::Out),
                PlayerRank::new("Dave", 0, PlayerRankStatus::Out),
            ]
        );
    }

    Ok(())
}

#[test]
fn test_rebalance_table() -> anyhow::Result<()> {
    let mtt_props = MttProperties {
        table_size: 6,
        blind_base: 50,
        blind_interval: 10000000,
        start_time: 0,
        ..Default::default()
    };

    let mut transactor = TestClient::transactor("Tx");
    let game_account = TestGameAccountBuilder::default()
        .with_data(mtt_props)
        .set_transactor(&transactor)
        .build();
    let mut ctx = GameContext::try_new(&game_account).unwrap();
    let mut handler = TestHandler::<Mtt>::init_state(&mut ctx, &game_account).unwrap();

    let p0 = TestClient::player("p0");
    let p1 = TestClient::player("p1");
    let p2 = TestClient::player("p2");
    let p3 = TestClient::player("p3");
    let p4 = TestClient::player("p4");
    let p5 = TestClient::player("p5");
    let p6 = TestClient::player("p6");
    let p7 = TestClient::player("p7");
    let p8 = TestClient::player("p8");
    let p9 = TestClient::player("p9");
    let pa = TestClient::player("pa");
    let pb = TestClient::player("pb");
    let pc = TestClient::player("pc");
    let pd = TestClient::player("pd");
    let pe = TestClient::player("pe");
    let pf = TestClient::player("pf");

    let sync_evt = create_sync_event(
        &ctx,
        &[
            &p0, &p1, &p2, &p3, &p4, &p5, &p6, &p7, &p8, &p9, &pa, &pb, &pc, &pd, &pe, &pf,
        ],
        &transactor,
    );

    handler.handle_until_no_events(&mut ctx, &sync_evt, vec![&mut transactor])?;

    let wait_timeout_evt = Event::WaitingTimeout;
    handler.handle_until_no_events(&mut ctx, &wait_timeout_evt, vec![&mut transactor])?;
    {
        let state = handler.get_state();
        assert_eq!(state.games.len(), 3);
        for game in state.games.values() {
            assert!(game.player_map.len() == 6 || game.player_map.len() == 5);
            assert_eq!(game.stage, HoldemStage::Play);
        }
        // Table assigns:
        // 1. 6 players: ["p6", "p9", "pc", "pf", "p0", "p3"]
        // 2. 5 players: ["p7", "pa", "pd", "p1", "p4"]
        // 3. 5 players: ["p8", "pb", "pe", "p2", "p5"]
    }

    // For table #1, kick out 3 players: pc, pf, p0
    let t1_allins = vec![
        pc.custom_event(GameEvent::Raise(10000)),
        pf.custom_event(GameEvent::Call),
        p0.custom_event(GameEvent::Call),
        p3.custom_event(GameEvent::Call),
        p6.custom_event(GameEvent::Fold),
        p9.custom_event(GameEvent::Fold),
    ];
    ctx.add_revealed_random(
        1,
        HashMap::from([
            (0, "sk".to_string()), // p0
            (1, "s2".to_string()),
            (2, "ca".to_string()), // p3
            (3, "da".to_string()),
            (4, "d7".to_string()), // p6
            (5, "c5".to_string()),
            (6, "h4".to_string()), // p9
            (7, "h6".to_string()),
            (8, "sa".to_string()), // pc
            (9, "sj".to_string()),
            (10, "da".to_string()), // pf
            (11, "dq".to_string()),
            (12, "st".to_string()), // board
            (13, "ct".to_string()),
            (14, "d4".to_string()),
            (15, "d8".to_string()),
            (16, "s9".to_string()),
        ]),
    )?;
    for evt in t1_allins {
        handler.handle_until_no_events(&mut ctx, &evt, vec![&mut transactor])?;
    }

    // For table #2 and #3, kick out 2 players from each
    // table #2: pd, p1
    // table #3: pe, p5
    let t2_allins = vec![
        pd.custom_event(GameEvent::Raise(10000)),
        p1.custom_event(GameEvent::Call),
        p4.custom_event(GameEvent::Call),
    ];
    ctx.add_revealed_random(
        2,
        HashMap::from([
            (0, "sk".to_string()), // p1
            (1, "s2".to_string()),
            (2, "ca".to_string()), // p4
            (3, "da".to_string()),
            (4, "d7".to_string()), // p7
            (5, "c5".to_string()),
            (6, "h4".to_string()), // pa
            (7, "h6".to_string()),
            (8, "sa".to_string()), // pd
            (9, "sj".to_string()),
            (10, "st".to_string()), // board
            (11, "ct".to_string()),
            (12, "d4".to_string()),
            (13, "d8".to_string()),
            (14, "s9".to_string()),
        ]),
    )?;

    ctx.add_revealed_random(
        3,
        HashMap::from([
            (0, "sk".to_string()), // p2
            (1, "s2".to_string()),
            (2, "ca".to_string()), // p5
            (3, "da".to_string()),
            (4, "d7".to_string()), // p8
            (5, "c5".to_string()),
            (6, "h4".to_string()), // pb
            (7, "h6".to_string()),
            (8, "sa".to_string()), // pe
            (9, "sj".to_string()),
            (10, "st".to_string()), // board
            (11, "ct".to_string()),
            (12, "d4".to_string()),
            (13, "d8".to_string()),
            (14, "s9".to_string()),
        ]),
    )?;

    Ok(())
}
