//! Test various types of Leave events.  It is crucial for players to
//! correctly leave the game.

mod helper;

use std::collections::HashMap;

use race_holdem_base::essential::*;
use helper::{create_sync_event, setup_holdem_game};
use race_api::{error::Result, event::Event};
use race_test::prelude::*;

// Two players leave one after another
#[test]
fn test_players_leave() -> Result<()> {
    let (_, mut game_acct, mut ctx, mut handler, mut transactor) = setup_holdem_game();
    let mut alice = TestClient::player("Alice");
    let mut bob = TestClient::player("Bob");

    let sync_evt = create_sync_event(&mut ctx, &mut game_acct, vec![&mut alice, &mut bob], &transactor);
    handler.handle_until_no_events(
        &mut ctx,
        &sync_evt,
        vec![&mut alice, &mut bob, &mut transactor],
    )?;

    {
        let state = handler.get_state();
        assert_eq!(
            state.player_order,
            vec![bob.id(), alice.id()]
        );
        assert_eq!(
            state.acting_player,
            Some(ActingPlayer {
                id: bob.id(),
                position: 1,
                clock: ACTION_TIMEOUT_POSTFLOP
            })
        );
        println!("-- Display {:?}", state.display);
        assert_eq!(state.display.len(), 1);
        assert!(state.display.contains(&Display::DealCards));
    }

    // Bob (SB/BTN) is the acting player and decides to leave
    let bob_leave = Event::Leave {
        player_id: bob.id(),
    };
    handler.handle_until_no_events(
        &mut ctx,
        &bob_leave,
        vec![&mut alice, &mut bob, &mut transactor],
    )?;

    {
        let state = handler.get_state();
        println!("-- Display {:?}", state.display);
        assert_eq!(state.stage, HoldemStage::Settle);
        assert_eq!(state.acting_player, None);
        assert_eq!(state.player_map.len(), 1);
        assert_eq!(
            state.player_map.get(&alice.id()).unwrap().status,
            PlayerStatus::Wait
        );
    }

    handler.handle_dispatch_event(&mut ctx)?;

    // Alice decides leaves as well
    let alice_leave = Event::Leave {
        player_id: alice.id()
    };
    handler.handle_until_no_events(
        &mut ctx,
        &alice_leave,
        vec![&mut alice, &mut bob, &mut transactor],
    )?;

    {
        let state = handler.get_state();
        assert_eq!(state.stage, HoldemStage::Init);
        // println!("-- State {:?}", state);
        // println!("-- Display {:?}", state.display);
        assert_eq!(state.player_map.len(), 0);
    }

    Ok(())
}

// Test one player leaving in settle
// Two players in game: Alice(BB) and Bob(SB/BTN)
// Bob folds then BB wins
// Bob leaves the game
// Expect Bob to leave instantly
#[test]
fn test_settle_leave() -> Result<()> {
    let (_, mut game_acct, mut ctx, mut handler, mut transactor) = setup_holdem_game();
    let mut alice = TestClient::player("Alice");
    let mut bob = TestClient::player("Bob");

    let sync_evt = create_sync_event(&mut ctx, &mut game_acct, vec![&mut alice, &mut bob], &transactor);
    handler.handle_until_no_events(
        &mut ctx,
        &sync_evt,
        vec![&mut alice, &mut bob, &mut transactor],
    )?;

    {
        let state = handler.get_state();
        assert_eq!(
            state.player_order,
            vec![bob.id(), alice.id()]
        );
        assert_eq!(
            state.acting_player,
            Some(ActingPlayer {
                id: bob.id(),
                position: 1,
                clock: ACTION_TIMEOUT_POSTFLOP
            })
        );
    }

    // Bob (SB/BTN) is the acting player and decides to fold
    let sb_fold = bob.custom_event(GameEvent::Fold);
    handler.handle_until_no_events(
        &mut ctx,
        &sb_fold,
        vec![&mut alice, &mut bob, &mut transactor],
    )?;

    // Alice (BB) should be Winner and game is in Settle
    {
        let state = handler.get_state();
        assert_eq!(state.stage, HoldemStage::Settle);
        assert_eq!(state.player_map.len(), 2);
        assert_eq!(
            state.player_map.get(&alice.id()).unwrap().status,
            PlayerStatus::Wait
        );
    }

    // Bob then decides to leave
    let sb_leave = Event::Leave {
        player_id: bob.id(),
    };
    handler.handle_until_no_events(
        &mut ctx,
        &sb_leave,
        vec![&mut alice, &mut bob, &mut transactor],
    )?;

    {
        let state = handler.get_state();
        assert_eq!(state.stage, HoldemStage::Settle);
        assert_eq!(state.acting_player, None);
        assert_eq!(state.player_map.len(), 1);
        assert!(state.player_map.contains_key(&alice.id()));
        println!("Game state {:?}", state);
    }

    Ok(())
}

// Test player leaving in runner
// Two players in the game: Alice(SB/BTN) and Bob(BB).
// Alice goes all-in, then Bob do a hero call.
// Alice leaves the game while the stage is Runner.
// Expect alice to leave instantly.
#[test]
fn test_runner_leave() -> Result<()> {
    let (_, mut game_acct, mut ctx, mut handler, mut transactor) = setup_holdem_game();
    let mut alice = TestClient::player("Alice");
    let mut bob = TestClient::player("Bob");
    let revealed = HashMap::from([
        // Bob
        (0, "ck".to_string()),
        (1, "ca".to_string()),
        // Alice
        (2, "c2".to_string()),
        (3, "c7".to_string()),
        // Board
        (4, "sa".to_string()),
        (5, "sk".to_string()),
        (6, "h3".to_string()),
        (7, "ha".to_string()),
        (8, "d4".to_string()),
    ]);

    let sync_evt = create_sync_event(&mut ctx, &mut game_acct, vec![&mut alice, &mut bob], &transactor);
    handler.handle_until_no_events(
        &mut ctx,
        &sync_evt,
        vec![&mut alice, &mut bob, &mut transactor],
    )?;

    {
        let state = handler.get_state();
        assert_eq!(
            state.player_order,
            vec![bob.id(), alice.id()]
        );
        assert_eq!(
            state.acting_player,
            Some(ActingPlayer {
                id: bob.id(),
                position: 1,
                clock: ACTION_TIMEOUT_POSTFLOP
            })
        );
    }
    ctx.add_revealed_random(1, revealed)?;

    // Bob (SB/BTN) is the acting player and decides to go allin
    let sb_allin = bob.custom_event(GameEvent::Raise(9990));
    handler.handle_until_no_events(
        &mut ctx,
        &sb_allin,
        vec![&mut alice, &mut bob, &mut transactor],
    )?;

    {
        let state = handler.get_state();
        assert_eq!(state.stage, HoldemStage::Play);
        assert_eq!(state.player_map.len(), 2);
        assert_eq!(
            state.player_map.get(&alice.id()).unwrap().status,
            PlayerStatus::Acting
        );
    }

    // BB makes a hero call
    let bb_herocall = alice.custom_event(GameEvent::Call);
    handler.handle_until_no_events(
        &mut ctx,
        &bb_herocall,
        vec![&mut alice, &mut bob, &mut transactor],
    )?;

    {
        let state = handler.get_state();
        assert_eq!(state.street, Street::Showdown);
        assert_eq!(state.stage, HoldemStage::Runner);
        assert_eq!(state.acting_player, None);
        assert_eq!(state.player_map.len(), 1);
    }

    // Alice then decides to leave
    let sb_leave = Event::Leave {
        player_id: alice.id(),
    };
    handler.handle_until_no_events(
        &mut ctx,
        &sb_leave,
        vec![&mut alice, &mut bob, &mut transactor],
    )?;

    {
        let state = handler.get_state();
        assert_eq!(state.stage, HoldemStage::Runner);
        assert_eq!(state.acting_player, None);
        assert_eq!(state.player_map.len(), 0);
        println!("Game state {:?}", state);
    }

    Ok(())
}

// Test player leaving in showdown
#[test]
fn test_showdown_leave() -> Result<()> {
    let (_, mut game_acct, mut ctx, mut handler, mut transactor) = setup_holdem_game();
    let mut alice = TestClient::player("Alice");
    let mut bob = TestClient::player("Bob");

    let sync_evt = create_sync_event(&mut ctx, &mut game_acct, vec![&mut alice, &mut bob], &transactor);
    handler.handle_until_no_events(
        &mut ctx,
        &sync_evt,
        vec![&mut alice, &mut bob, &mut transactor],
    )?;

    {
        let state = handler.get_state();
        assert_eq!(
            state.player_order,
            vec![bob.id(), alice.id()]
        );
        assert_eq!(
            state.acting_player,
            Some(ActingPlayer {
                id: bob.id(),
                position: 1,
                clock: ACTION_TIMEOUT_POSTFLOP
            })
        );
    }

    // Bob (SB/BTN) is the acting player and calls
    let sb_call = bob.custom_event(GameEvent::Call);
    handler.handle_until_no_events(
        &mut ctx,
        &sb_call,
        vec![&mut alice, &mut bob, &mut transactor],
    )?;

    {
        let state = handler.get_state();
        assert_eq!(state.stage, HoldemStage::Play);
        assert_eq!(state.street, Street::Preflop);
        assert_eq!(state.player_map.len(), 2);
        assert_eq!(
            state.player_map.get(&alice.id()).unwrap().status,
            PlayerStatus::Acting
        );
    }

    // Alice decides to check and street --> Flop
    let bb_check = alice.custom_event(GameEvent::Check);
    handler.handle_until_no_events(
        &mut ctx,
        &bb_check,
        vec![&mut alice, &mut bob, &mut transactor],
    )?;

    {
        let state = handler.get_state();
        assert_eq!(state.stage, HoldemStage::Play);
        assert_eq!(state.street, Street::Flop);
        // Acting player is now Alice
        assert_eq!(
            state.player_map.get(&alice.id()).unwrap().status,
            PlayerStatus::Acting
        );
    }

    // From this point on, two players keep checking until showdown
    // Flop -> Turn
    let sb_check = alice.custom_event(GameEvent::Check);
    handler.handle_until_no_events(
        &mut ctx,
        &sb_check,
        vec![&mut alice, &mut bob, &mut transactor],
    )?;

    let bb_check = bob.custom_event(GameEvent::Check);
    handler.handle_until_no_events(
        &mut ctx,
        &bb_check,
        vec![&mut alice, &mut bob, &mut transactor],
    )?;

    {
        let state = handler.get_state();
        assert_eq!(state.stage, HoldemStage::Play);
        assert_eq!(state.street, Street::Turn);
        assert_eq!(
            state.player_map.get(&alice.id()).unwrap().status,
            PlayerStatus::Acting
        );
    }

    // Turn -> River
    let sb_check = alice.custom_event(GameEvent::Check);
    handler.handle_until_no_events(
        &mut ctx,
        &sb_check,
        vec![&mut alice, &mut bob, &mut transactor],
    )?;
    let bb_check = bob.custom_event(GameEvent::Check);
    handler.handle_until_no_events(
        &mut ctx,
        &bb_check,
        vec![&mut alice, &mut bob, &mut transactor],
    )?;
    {
        let state = handler.get_state();
        assert_eq!(state.stage, HoldemStage::Play);
        assert_eq!(state.street, Street::River);
        assert_eq!(
            state.acting_player,
            Some(ActingPlayer {
                id: alice.id(),
                position: 0,
                clock: 30_000
            })
        );
    }

    // River -> Showdown
    let sb_check = alice.custom_event(GameEvent::Check);
    handler.handle_until_no_events(
        &mut ctx,
        &sb_check,
        vec![&mut alice, &mut bob, &mut transactor],
    )?;
    let bb_check = bob.custom_event(GameEvent::Check);
    handler.handle_until_no_events(
        &mut ctx,
        &bb_check,
        vec![&mut alice, &mut bob, &mut transactor],
    )?;
    {
        let state = handler.get_state();
        assert_eq!(state.stage, HoldemStage::Showdown);
        assert_eq!(state.street, Street::Showdown);
        assert_eq!(state.acting_player, None);
    }

    // Alice decides to leave
    let sb_leave = Event::Leave {
        player_id: alice.id(),
    };
    handler.handle_until_no_events(
        &mut ctx,
        &sb_leave,
        vec![&mut alice, &mut bob, &mut transactor],
    )?;
    {
        let state = handler.get_state();
        assert_eq!(state.stage, HoldemStage::Showdown);
        assert_eq!(state.player_map.len(), 1);
        assert!(!state.player_map.contains_key(&alice.id()));
    }

    Ok(())
}

/// Test player leave in a multi-players game.
/// Three players: Alice, Bob and Charlie
/// Alice leave in the middle of the game when she's acting,
/// Bob and Charlie check to showdown.
#[test]
fn test_leave_in_multiplayers() -> Result<()> {
    let (_, mut game_acct, mut ctx, mut handler, mut transactor) = setup_holdem_game();
    let mut alice = TestClient::player("Alice");
    let mut bob = TestClient::player("Bob");
    let mut charlie = TestClient::player("Charlie");

    let sync_evt = create_sync_event(&mut ctx, &mut game_acct, vec![&mut alice, &mut bob, &mut charlie], &transactor);
    handler.handle_until_no_events(
        &mut ctx,
        &sync_evt,
        vec![&mut alice, &mut bob, &mut charlie, &mut transactor],
    )?;

    {
        let state = handler.get_state();
        assert_eq!(
            state.player_order,
            vec![bob.id(), charlie.id(), alice.id()]
        );
        assert_eq!(
            state.acting_player,
            Some(ActingPlayer {
                id: bob.id(),
                position: 1,
                clock: ACTION_TIMEOUT_POSTFLOP,
            })
        );
    }

    let bob_call = bob.custom_event(GameEvent::Call);
    handler.handle_until_no_events(
        &mut ctx,
        &bob_call,
        vec![&mut alice, &mut bob, &mut charlie, &mut transactor]
    )?;
    let alice_leave = Event::Leave {
        player_id: alice.id(),
    };

    {
        let state = handler.get_state();
        assert_eq!(
            state.street,
            Street::Preflop
        );
    }

    handler.handle_until_no_events(
        &mut ctx,
        &alice_leave,
        vec![&mut alice, &mut bob, &mut charlie, &mut transactor]
    )?;

    let charlie_call = charlie.custom_event(GameEvent::Call);

    handler.handle_until_no_events(
        &mut ctx,
        &charlie_call,
        vec![&mut alice, &mut bob, &mut charlie, &mut transactor]
    )?;

    let charlie_check = charlie.custom_event(GameEvent::Check);
    handler.handle_until_no_events(
        &mut ctx,
        &charlie_check,
        vec![&mut alice, &mut bob, &mut charlie, &mut transactor]
    )?;

    let bob_check = bob.custom_event(GameEvent::Check);
    handler.handle_until_no_events(
        &mut ctx,
        &bob_check,
        vec![&mut alice, &mut bob, &mut charlie, &mut transactor]
    )?;

    let charlie_check = charlie.custom_event(GameEvent::Check);
    handler.handle_until_no_events(
        &mut ctx,
        &charlie_check,
        vec![&mut alice, &mut bob, &mut charlie, &mut transactor]
    )?;

    let bob_check = bob.custom_event(GameEvent::Check);
    handler.handle_until_no_events(
        &mut ctx,
        &bob_check,
        vec![&mut alice, &mut bob, &mut charlie, &mut transactor]
    )?;

    let charlie_check = charlie.custom_event(GameEvent::Check);
    handler.handle_until_no_events(
        &mut ctx,
        &charlie_check,
        vec![&mut alice, &mut bob, &mut charlie, &mut transactor]
    )?;

    let bob_check = bob.custom_event(GameEvent::Check);
    handler.handle_until_no_events(
        &mut ctx,
        &bob_check,
        vec![&mut alice, &mut bob, &mut charlie, &mut transactor]
    )?;

    Ok(())
}

#[test]
fn test_play_leave() -> Result<()> {
    let (_, mut game_acct, mut ctx, mut handler, mut transactor) = setup_holdem_game();
    let mut alice = TestClient::player("Alice");
    let mut bob = TestClient::player("Bob");

    let sync_evt = create_sync_event(&mut ctx, &mut game_acct, vec![&mut alice, &mut bob], &transactor);
    handler.handle_until_no_events(
        &mut ctx,
        &sync_evt,
        vec![&mut alice, &mut bob, &mut transactor],
    )?;

    {
        let state = handler.get_state();
        assert_eq!(
            state.player_order,
            vec![bob.id(), alice.id()]
        );
        assert_eq!(
            state.acting_player,
            Some(ActingPlayer {
                id: bob.id(),
                position: 1,
                clock: ACTION_TIMEOUT_POSTFLOP
            })
        );
    }

    // Bob (SB/BTN) is the acting player and decides to fold
    let sb_fold = bob.custom_event(GameEvent::Fold);
    handler.handle_until_no_events(
        &mut ctx,
        &sb_fold,
        vec![&mut alice, &mut bob, &mut transactor],
    )?;

    // Alice (BB) should be Winner and game is in Settle
    {
        let state = handler.get_state();
        assert_eq!(state.stage, HoldemStage::Settle);
        assert_eq!(state.player_map.len(), 2);
        assert_eq!(
            state.player_map.get(&alice.id()).unwrap().status,
            PlayerStatus::Wait
        );
    }

    // Alice then decides to leave
    let bb_leave = Event::Leave {
        player_id: alice.id()
    };
    handler.handle_until_no_events(
        &mut ctx,
        &bb_leave,
        vec![&mut alice, &mut bob, &mut transactor],
    )?;

    {
        let state = handler.get_state();
        assert_eq!(state.stage, HoldemStage::Settle);
        assert_eq!(state.acting_player, None);
        assert_eq!(state.player_map.len(), 1);
        assert!(state.player_map.contains_key(&bob.id()));
        println!("Game state {:?}", state);
    }

    Ok(())
}
