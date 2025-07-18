//! Test for the scenario where game does not start with 2 players:
//! player A with status `Waitbb` and player B with status `Wait`
//! A extra player has just left (or got kicked out of) the game

mod helper;
use helper::setup_holdem_game;
use race_holdem_base::game::Holdem;
use race_api::prelude::*;
use race_holdem_base::essential::*;
use race_test::prelude::*;
use race_holdem_base::hand_history::HandHistory;
use std::collections::BTreeMap;

// In the previous buggy version, we shoud expect this to panic because
// when there is only one player with `Wait` status (as sb), accessing bb
// will cause out of bound error.
//
// In the fixed version, this test still fails (intentionlly) as the player
// with `Waitbb` status should be updated to `Wait`
#[test]
fn test_only_one_wait_player() -> HandleResult<()> {
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

    // BTN(carol) and SB(alice) choose to leave and BB(bob) wins
    let leaves = vec![
        Event::Leave {player_id: carol.id()},
        Event::Leave {player_id: alice.id()},
    ];
    ctx.handle_multiple_events(&leaves)?;
    ctx.handle_dispatch_until_no_events(vec![&mut alice, &mut bob, &mut carol, &mut dave, &mut tx])?;

    // Two players left, bob and dave. bob will be BTN(SB) and act first.
    {
        let state = ctx.state();
        let dave = state.player_map.get(&dave.id()).unwrap();
        assert_eq!(dave.status, PlayerStatus::Wait);
        assert_eq!(state.street, Street::Preflop);
        assert!(state.is_acting_player(bob.id()));
    }

    Ok(())
}
