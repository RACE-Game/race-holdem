//! Test various situations where one or more players go all in
mod helper;

use race_api::prelude::*;
use race_test::prelude::*;
use std::collections::HashMap;

use helper::{create_sync_event, setup_holdem_game};
use race_holdem_base::essential::*;

// One player goes all in early and the rest keep playing until showdown
#[test]
fn test_allin_case1() -> Result<()> {
    let (_game_acct, mut ctx, mut handler, mut transactor) = setup_holdem_game();

    let mut alice = TestClient::player("Alice");
    let mut bob = TestClient::player("Bob");
    let mut carol = TestClient::player("Carol");
    let mut dave = TestClient::player("Dave");

    let mut sync_evt = create_sync_event(&ctx, &[&alice, &bob, &carol, &dave], &transactor);

    {
        match &mut sync_evt {
            Event::Sync { new_players, .. } => {
                new_players[0].balance = 666;
                new_players[1].balance = 777;
                new_players[2].balance = 999;
                new_players[3].balance = 888;
            }
            _ => (),
        }
    }

    handler.handle_until_no_events(
        &mut ctx,
        &sync_evt,
        vec![&mut alice, &mut bob, &mut carol, &mut dave, &mut transactor],
    )?;

    // Game starts
    {
        let runner_revealed = HashMap::from([
            // Alice
            (0, "sk".to_string()),
            (1, "ck".to_string()),
            // Bob
            (2, "ht".to_string()),
            (3, "dt".to_string()),
            // Carol
            (4, "h8".to_string()),
            (5, "c8".to_string()),
            // Dave
            (6, "s2".to_string()),
            (7, "c7".to_string()),
            // Board
            (8, "s5".to_string()),
            (9, "c6".to_string()),
            (10, "h2".to_string()),
            (11, "h9".to_string()),
            (12, "d7".to_string()),
        ]);
        let state = handler.get_state();
        ctx.add_revealed_random(state.deck_random_id, runner_revealed)?;
        println!("Player order {:?}", state.player_order);
    }

    let evts = [
        // Preflop
        alice.custom_event(GameEvent::Raise(111)),
        bob.custom_event(GameEvent::Call),
        carol.custom_event(GameEvent::Raise(222)),
        dave.custom_event(GameEvent::Call),
        alice.custom_event(GameEvent::Raise(555)), // Allin
        bob.custom_event(GameEvent::Fold),
        carol.custom_event(GameEvent::Call),
        dave.custom_event(GameEvent::Call),
        // Flop
        carol.custom_event(GameEvent::Check),
        dave.custom_event(GameEvent::Check),
        // Turn
        carol.custom_event(GameEvent::Check),
        dave.custom_event(GameEvent::Check),
        // River
        carol.custom_event(GameEvent::Bet(111)),
        dave.custom_event(GameEvent::Call),
    ];

    for evt in evts {
        handler.handle_until_no_events(
            &mut ctx,
            &evt,
            vec![&mut alice, &mut bob, &mut carol, &mut dave, &mut transactor],
        )?;
    }

    {
        let state = handler.get_state();
        assert_eq!(state.street, Street::Showdown);
    }
    Ok(())
}

#[test]
fn test_allin_case2() -> Result<()> {
    let (_game_acct, mut ctx, mut handler, mut transactor) = setup_holdem_game();

    let mut alice = TestClient::player("Alice");
    let mut bob = TestClient::player("Bob");
    let mut carol = TestClient::player("Carol");
    let mut dave = TestClient::player("Dave");
    let mut frank = TestClient::player("Frank");

    let mut sync_evt = create_sync_event(&ctx, &[&alice, &bob, &carol, &dave, &frank], &transactor);

    {
        match &mut sync_evt {
            Event::Sync { new_players, .. } => {
                new_players[0].balance = 1000;
                new_players[1].balance = 1000;
                new_players[2].balance = 1000;
                new_players[3].balance = 1000;
                new_players[4].balance = 1000;
            }
            _ => (),
        }
    }

    handler.handle_until_no_events(&mut ctx, &sync_evt, vec![&mut transactor])?;

    // Game starts
    {
        let runner_revealed = HashMap::from([
            // Alice
            (0, "sk".to_string()),
            (1, "ck".to_string()),
            // Bob
            (2, "ht".to_string()),
            (3, "dt".to_string()),
            // Carol
            (4, "h8".to_string()),
            (5, "c8".to_string()),
            // Dave
            (6, "s2".to_string()),
            (7, "c7".to_string()),
            // Frank
            (8, "s4".to_string()),
            (9, "c9".to_string()),
            // Board
            (10, "s5".to_string()),
            (11, "c6".to_string()),
            (12, "h2".to_string()),
            (13, "h9".to_string()),
            (14, "d7".to_string()),
        ]);
        let state = handler.get_state();
        ctx.add_revealed_random(state.deck_random_id, runner_revealed)?;
        println!("Player order {:?}", state.player_order);
    }

    let evts = [
        frank.custom_event(GameEvent::Fold),
        alice.custom_event(GameEvent::Raise(1000)),
        bob.custom_event(GameEvent::Call),
        carol.custom_event(GameEvent::Call),
        dave.custom_event(GameEvent::Call),
    ];

    for evt in evts {
        handler.handle_until_no_events(
            &mut ctx,
            &evt,
            vec![
                &mut alice,
                &mut bob,
                &mut carol,
                &mut dave,
                &mut frank,
                &mut transactor,
            ],
        )?;
    }

    {
        let state = handler.get_state();
        assert_eq!(state.street, Street::Showdown);
    }

    Ok(())
}
