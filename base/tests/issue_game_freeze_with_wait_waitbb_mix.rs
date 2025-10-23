//! Test for the scenario where game has only 2 players:
//! 1. player A with status `Waitbb`
//! 2. player B with status `Wait`
//! This can happen due to the fact that other players have
//! just left (or got kicked out of) the game

mod helper;
use helper::setup_holdem_game;
use race_api::prelude::*;
use race_poker_base::essential::*;
use race_test::prelude::*;

// In the previous buggy version, we shoud expect this to panic because
// when there is only one player with `Wait` status (as sb), accessing bb
// will cause out of bound error.
//
// In the fixed version, this test should make `Wait` player the sb/btn,
// and update `Waitbb` player with `Wait` and make him the bb
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
    // dave should be the bb
    {
        let state = ctx.state();
        let player_bob = state.player_map.get(&bob.id()).unwrap();
        let player_dave = state.player_map.get(&dave.id()).unwrap();
        assert_eq!(player_dave.status, PlayerStatus::Wait);
        assert_eq!(state.street, Street::Preflop);
        assert_eq!(state.bet_map.get(&dave.id()), Some(&20));
        assert_eq!(state.bet_map.get(&bob.id()), Some(&10));
        assert_eq!(state.btn, player_bob.position);
        assert!(state.is_acting_player(bob.id()));
    }

    Ok(())
}
