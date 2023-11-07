//! Test handling various events in Holdem.  There are two types of events:
//! 1. General events such as Sync, GameStart, WaitTimeout, etc;
//! 2. Custom events that are exclusively relevant to Holdem:
//! Call, Bet, Raise, Leave, etc.

mod helper;

use race_api::error::Result as CoreResult;
use race_test::prelude::*;
use helper::{create_sync_event, setup_holdem_game};
use race_holdem_base::essential::*;

#[test]
fn test_preflop_fold() -> CoreResult<()> {
    let (_game_acct, mut ctx, mut handler, mut transactor) = setup_holdem_game();

    let mut alice = TestClient::player("Alice");
    let mut bob = TestClient::player("Bob");

    let sync_evt = create_sync_event(&ctx, &[&alice, &bob], &transactor);

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
        assert!(state.is_acting_player(&"Alice".to_string()));
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
        let alice = state.player_map.get("Alice").unwrap();
        let bob = state.player_map.get("Bob").unwrap();
        // Street should remain unchanged
        assert_eq!(state.street, Street::Preflop);
        assert_eq!(alice.chips, 9990);
        assert_eq!(bob.chips, 10_010);
        assert_eq!(
            state.player_map.get("Bob").unwrap().status,
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
