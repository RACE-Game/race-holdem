//! Test various of Sync event in the middle of the game

mod helper;

use helper::{create_sync_event, setup_holdem_game};
use race_api::error::Result;
use race_holdem_base::essential::*;
use race_test::prelude::*;

// The game starts with two player
// The third player joins in the preflop
// Then both players keep checking to showdown
#[test]
fn test_join_on_preflop() -> Result<()> {
    let (_game_acct, mut ctx, mut handler, mut transactor) = setup_holdem_game();
    let mut alice = TestClient::player("Alice");
    let mut bob = TestClient::player("Bob");
    let mut charlie = TestClient::player("Charlie");

    let sync_evt = create_sync_event(&ctx, &[&alice, &bob], &transactor);
    handler.handle_until_no_events(
        &mut ctx,
        &sync_evt,
        vec![&mut alice, &mut bob, &mut transactor],
    )?;

    {
        let state = handler.get_state();
        assert_eq!(state.street, Street::Preflop);
        assert_eq!(
            state.player_order,
            vec!["Bob".to_string(), "Alice".to_string()]
        );
    }

    let charlie_sync_evt = create_sync_event(&ctx, &[&charlie], &transactor);
    handler.handle_until_no_events(
        &mut ctx,
        &charlie_sync_evt,
        vec![&mut alice, &mut bob, &mut charlie, &mut transactor],
    )?;

    {
        let state = handler.get_state();
        assert!(state
            .player_map
            .get("Charlie")
            .is_some_and(|p| p.status == PlayerStatus::Init));
    }

    // Check till the end
    let evts = [
        bob.custom_event(GameEvent::Call),
        alice.custom_event(GameEvent::Check),
        alice.custom_event(GameEvent::Check),
        bob.custom_event(GameEvent::Check),
        alice.custom_event(GameEvent::Check),
        bob.custom_event(GameEvent::Check),
        alice.custom_event(GameEvent::Check),
        bob.custom_event(GameEvent::Check),
    ];
    for evt in evts {
        handler.handle_until_no_events(
            &mut ctx,
            &evt,
            vec![&mut alice, &mut bob, &mut charlie, &mut transactor],
        )?;
    }

    Ok(())
}

// The game starts with two player
// The third player joins in the preflop
// Then both players keep checking to showdown
#[test]
fn test_on_preflop_then_runner() -> Result<()> {
    let (_game_acct, mut ctx, mut handler, mut transactor) = setup_holdem_game();
    let mut alice = TestClient::player("Alice");
    let mut bob = TestClient::player("Bob");
    let mut charlie = TestClient::player("Charlie");

    let sync_evt = create_sync_event(&ctx, &[&alice, &bob], &transactor);

    handler.handle_until_no_events(
        &mut ctx,
        &sync_evt,
        vec![&mut alice, &mut bob, &mut transactor],
    )?;

    {
        let state = handler.get_state();
        assert_eq!(state.street, Street::Preflop);
        assert_eq!(
            state.player_order,
            vec!["Bob".to_string(), "Alice".to_string()]
        );
    }

    let charlie_sync_evt = create_sync_event(&ctx, &[&charlie], &transactor);
    handler.handle_until_no_events(
        &mut ctx,
        &charlie_sync_evt,
        vec![&mut alice, &mut bob, &mut charlie, &mut transactor],
    )?;

    {
        let state = handler.get_state();
        assert!(state
            .player_map
            .get("Charlie")
            .is_some_and(|p| p.status == PlayerStatus::Init));
    }

    // Bob goes allin and Alice calls

    let evts = [
        bob.custom_event(GameEvent::Raise(9990)),
        alice.custom_event(GameEvent::Call),
    ];
    for evt in evts {
        handler.handle_until_no_events(
            &mut ctx,
            &evt,
            vec![&mut alice, &mut bob, &mut charlie, &mut transactor],
        )?;
    }

    {
        let state = handler.get_state();
        assert_eq!(state.stage, HoldemStage::Runner);
    }
    Ok(())
}
