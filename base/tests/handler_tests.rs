//! Test handling various events in Holdem.  There are two types of events:
//! 1. General events such as Sync, GameStart, WaitTimeout, etc;
//! 2. Custom events that are exclusively relevant to Holdem:
//! Call, Bet, Raise, Leave, etc.

mod helper;

use std::collections::HashMap;

use helper::{create_sync_event, setup_holdem_game};
use race_api::{error::Result as CoreResult, prelude::Event};
use race_holdem_base::essential::*;
use race_test::prelude::*;

#[test]
fn test_preflop_fold() -> CoreResult<()> {
    let (_, mut game_acct, mut ctx, mut handler, mut transactor) = setup_holdem_game();

    let mut alice = TestClient::player("Alice");
    let mut bob = TestClient::player("Bob");

    let sync_evt = create_sync_event(&mut ctx, &mut game_acct, vec![&mut alice, &mut bob], &transactor);

    {
        let state = handler.get_mut_state();
        state.btn = 2;
    }

    handler.handle_until_no_events(
        &mut ctx,
        &sync_evt,
        vec![&mut alice, &mut bob, &mut transactor],
    )?;

    // Regular tests to make sure holdem has been set up properly
    {
        let state = handler.get_state();
        assert_eq!(state.street, Street::Preflop);
        assert_eq!(state.btn, 0);
        assert!(state.is_acting_player(alice.id()));
    }

    // SB(Alice) folds so BB(Bob), the single player, wins
    let alice_fold = alice.custom_event(GameEvent::Fold);
    handler.handle_until_no_events(
        &mut ctx,
        &alice_fold,
        vec![&mut alice, &mut bob, &mut transactor],
    )?;

    {
        let state = handler.get_state();
        let alice = state.player_map.get(&alice.id()).unwrap();
        let bob = state.player_map.get(&bob.id()).unwrap();
        // Street should remain unchanged
        assert_eq!(state.street, Street::Preflop);
        assert_eq!(alice.chips, 9990);
        assert_eq!(bob.chips, 10_010);
        assert_eq!(
            state.player_map.get(&bob.id()).unwrap().status,
            PlayerStatus::Wait
        );
    }

    // Game should be able to start again with BTN changed
    handler.handle_dispatch_event(&mut ctx)?; // WaitingTimeout
    handler.handle_dispatch_event(&mut ctx)?; // GameStart
    {
        let state = handler.get_state();
        assert_eq!(state.btn, 1);
    }

    Ok(())
}

#[test]
fn test_2() -> Result<()> {
    let (_, mut game_acct, mut ctx, mut handler, mut transactor) = setup_holdem_game();

    let mut alice = TestClient::player("Alice");
    let mut bob = TestClient::player("Bob");
    let mut carol = TestClient::player("Carol");

    let mut sync_evt = create_sync_event(&mut ctx, &mut game_acct, vec![&mut alice, &mut bob, &mut carol], &transactor);

    {
        match &mut sync_evt {
            Event::Sync { new_players, .. } => {
                new_players[0].balance = 83380001;
                new_players[1].balance = 212870000;
                new_players[2].balance = 375929168;
            }
            _ => (),
        }
        let state = handler.get_mut_state();
        state.btn = 1;
        state.rake = 3;
    }

    handler.handle_until_no_events(
        &mut ctx,
        &sync_evt,
        vec![&mut alice, &mut bob, &mut carol, &mut transactor],
    )?;

    {
        let state = handler.get_mut_state();
        assert_eq!(state.btn, 2);
        assert_eq!(
            state
            .acting_player
            .as_ref()
            .and_then(|a| Some(a.id)),
            Some(carol.id())
        );

        let runner_revealed = HashMap::from([
            // Alice
            (0, "st".to_string()),
            (1, "ct".to_string()),
            // Bob
            (2, "ht".to_string()),
            (3, "dt".to_string()),
            // Carol
            (4, "h7".to_string()),
            (5, "d2".to_string()),
            // Board
            (6, "s5".to_string()),
            (7, "c6".to_string()),
            (8, "h2".to_string()),
            (9, "h8".to_string()),
            (10, "d7".to_string()),
        ]);
        let holdem_state = handler.get_state();
        ctx.add_revealed_random(holdem_state.deck_random_id, runner_revealed)?;
    }

    let evts = [
        carol.custom_event(GameEvent::Fold),
        alice.custom_event(GameEvent::Call),
        bob.custom_event(GameEvent::Check),
        alice.custom_event(GameEvent::Check),
        bob.custom_event(GameEvent::Check),
        alice.custom_event(GameEvent::Bet(750000)),
        bob.custom_event(GameEvent::Raise(212870000)),
        alice.custom_event(GameEvent::Call),
    ];

    for evt in evts {
        handler.handle_until_no_events(
            &mut ctx,
            &evt,
            vec![&mut alice, &mut bob, &mut carol, &mut transactor],
        )?;
    }

    {
        let state = handler.get_state();
        assert_eq!(state.stage, HoldemStage::Runner);
    }

    Ok(())
}
