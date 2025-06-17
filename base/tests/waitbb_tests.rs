//! Test new players joining a cash table to get `Waitbb` status and to start
//! playing only when they can become the big blind for the next hand.

mod helper;
use helper::setup_holdem_game;
use race_api::{error::HandleResult, prelude::Event};
use race_holdem_base::essential::*;
use race_test::prelude::*;

// All players join the game sequentially and no one leaves or gets kicked out.
// Thus the new players (with status Waitbb) always take positions after the
// existing ones.  This tests a table with more than 2 players.
#[test]
fn test_sequential_waitbb() -> HandleResult<()> {
    let mut tx = TestClient::transactor("tx");

    let mut ctx = setup_holdem_game(&mut tx);

    let mut alice = TestClient::player("alice"); // pos: 0, BB at first
    let mut bob = TestClient::player("bob");     // pos: 1, btn at first
    let mut carol = TestClient::player("carol"); // pos: 2, SB at first
    let mut dave = TestClient::player("dave");   // not joined at first

    let (join, deposit) = ctx.join_multi(vec![(&mut alice, 1000), (&mut bob, 1000), (&mut carol, 1000)]);
    ctx.handle_multiple_events(&[join, deposit])?;
    ctx.handle_dispatch_until_no_events(
        vec![&mut alice, &mut bob, &mut carol, &mut tx],
    )?;

    {
        let state = ctx.state();
        assert_eq!(state.street, Street::Preflop);
        assert_eq!(state.btn, 1);
        assert!(state.is_acting_player(bob.id()));
    }

    //  a new player, dave, joins and the status is Waitbb
    let (djoin, ddeposit) = ctx.join(&mut dave, 1000);
    ctx.handle_multiple_events(&[djoin, ddeposit])?;
    // BTN(bob) folds, SB(carol) folds, BB(alice) wins
    let bob_fold = bob.custom_event(GameEvent::Fold);
    let carol_fold = carol.custom_event(GameEvent::Fold);
    ctx.handle_multiple_events(&[bob_fold, carol_fold])?;
    ctx.handle_dispatch_until_no_events(vec![&mut alice, &mut bob, &mut carol, &mut dave, &mut tx])?;

    // Dave needs to wait because SB < BB < NP
    {
        let state = ctx.state();
        let dave = state.player_map.get(&dave.id()).unwrap();
        assert_eq!(state.btn, 2);
        assert_eq!(dave.status, PlayerStatus::Waitbb);
        assert_eq!(state.street, Street::Preflop);
        assert!(state.is_acting_player(carol.id()));
    }

    // BTN(carol) folds, SB(alice) folds and BB(bob) wins
    let carol_fold2 = carol.custom_event(GameEvent::Fold);
    let alice_fold2 = alice.custom_event(GameEvent::Fold);
    ctx.handle_multiple_events(&[carol_fold2, alice_fold2])?;
    ctx.handle_dispatch_until_no_events(vec![&mut alice, &mut bob, &mut carol, &mut dave, &mut tx])?;

    // Dave still needs to wait because SB < BB < NP
    {
        let state = ctx.state();
        let dave = state.player_map.get(&dave.id()).unwrap();
        assert_eq!(state.btn, 0);
        assert_eq!(dave.status, PlayerStatus::Waitbb);
        assert_eq!(state.street, Street::Preflop);
        assert!(state.is_acting_player(alice.id()));
    }

    // BTN(alice) folds, SB(bob) folds and BB(bob) wins
    let alice_fold3 = alice.custom_event(GameEvent::Fold);
    let bob_fold3 = bob.custom_event(GameEvent::Fold);
    ctx.handle_multiple_events(&[alice_fold3, bob_fold3])?;
    ctx.handle_dispatch_until_no_events(vec![&mut alice, &mut bob, &mut carol, &mut dave, &mut tx])?;

    // Dave now should join and become the actual BB because: SB > BB && NP > SB
    {
        let state = ctx.state();
        let dave = state.player_map.get(&dave.id()).unwrap();
        assert_eq!(state.btn, 1);
        assert_eq!(dave.status, PlayerStatus::Wait);
        assert_eq!(state.street, Street::Preflop);
        assert!(state.is_acting_player(alice.id()));
    }

    Ok(())
}
