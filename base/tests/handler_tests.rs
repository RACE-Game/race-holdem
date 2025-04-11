//! Test handling various events in Holdem.  There are two types of events:
//! 1. General events such as Sync, GameStart, WaitTimeout, etc;
//! 2. Custom events that are exclusively relevant to Holdem:
//! Call, Bet, Raise, Leave, etc.

mod helper;

use helper::setup_holdem_game;
use race_api::{error::HandleResult, prelude::Event};
use race_holdem_base::essential::*;
use race_test::prelude::*;

#[test]
fn test_preflop_fold() -> HandleResult<()> {

    let mut tx = TestClient::transactor("tx");

    let mut ctx = setup_holdem_game(&mut tx);

    let mut alice = TestClient::player("alice");
    let mut bob = TestClient::player("bob");

    let join_and_deposit = ctx.join_multi(vec![(&mut alice, 1000), (&mut bob, 1000)]);

    ctx.handle_event_multi(&join_and_deposit)?;

    {
        assert_eq!(ctx.current_dispatch(), Some(DispatchEvent::new(Event::GameStart, 0)));
    }

    ctx.handle_dispatch_until_no_events(
        vec![&mut alice, &mut bob, &mut tx],
    )?;

    // Regular tests to make sure holdem has been set up properly
    {
        let state = ctx.state();
        assert_eq!(state.street, Street::Preflop);
        assert_eq!(state.btn, 0);
        assert!(state.is_acting_player(alice.id()));
    }

    // SB(Alice) folds so BB(Bob), the single player, wins
    let alice_fold = alice.custom_event(GameEvent::Fold);
    ctx.handle_event(&alice_fold)?;
    ctx.handle_dispatch_until_no_events(vec![&mut alice, &mut bob, &mut tx])?;

    {
        let state = ctx.state();
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
    ctx.handle_dispatch_event()?; // WaitingTimeout
    ctx.handle_dispatch_event()?; // GameStart
    {
        let state = ctx.state();
        assert_eq!(state.btn, 1);
    }

    Ok(())
}
