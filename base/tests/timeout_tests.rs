mod helper;

use helper::{create_sync_event, setup_holdem_game};
use race_api::{error::Result, event::Event};
use race_holdem_base::essential::*;
use race_test::prelude::*;

// Test one player reaches the maximum number of timeouts in heads up
#[test]
fn test_headsup_action_timeout() -> Result<()> {
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
        let state = handler.get_mut_state();
        state
            .player_map
            .entry(bob.id())
            .and_modify(|p| p.timeout = MAX_ACTION_TIMEOUT_COUNT);
    }

    // Bob's ActionTimeout event
    handler.handle_dispatch_event(&mut ctx)?;

    {
        let state = handler.get_state();
        assert_eq!(state.player_map.len(), 1);
    }
    Ok(())
}

#[test]
fn test_multiplayers_consecutive_timeout() -> Result<()> {
    let (_, mut game_acct, mut ctx, mut handler, mut transactor) = setup_holdem_game();
    let mut alice = TestClient::player("Alice");
    let mut bob = TestClient::player("Bob");
    let mut charlie = TestClient::player("Charlie");
    let mut dave = TestClient::player("Dave");
    let mut eve = TestClient::player("Eve");
    let mut frank = TestClient::player("Frank");
    let mut grace = TestClient::player("Grace");

    // For sake of convenience, assume all players join the game at the same time
    let sync_evt = create_sync_event(
        &mut ctx,
        &mut game_acct,
        vec![&mut alice, &mut bob, &mut charlie, &mut dave, &mut eve, &mut frank, &mut grace],
        &transactor,
    );

    handler.handle_until_no_events(
        &mut ctx,
        &sync_evt,
        vec![
            &mut alice,   // 0
            &mut bob,     // 1
            &mut charlie, // 2
            &mut dave,    // 3
            &mut eve,     // 4
            &mut frank,   // 5
            &mut grace,   // 6
            &mut transactor,
        ],
    )?;

    {
        let state = handler.get_state();
        // BTN == position 1, SB == position 2, BB == position 3
        assert_eq!(state.player_map.len(), 7);
        assert_eq!(
            state.player_order,
            vec![
                eve.id(),
                frank.id(),
                grace.id(),
                alice.id(),
                bob.id(),
                charlie.id(),
                dave.id(),
            ]
        );
        assert_eq!(state.street, Street::Preflop);
        assert_eq!(
            state.acting_player,
            Some(ActingPlayer {
                id: eve.id(),
                position: 4,
                clock: 15000
            })
        );

        for p in state.player_map.values() {
            println!("Player {} with status {:?}", p.id, p.status);
        }
    }

    // Let players act timeout one by one
    let action_timeout = Event::ActionTimeout {
        player_id: eve.id(),
    };
    handler.handle_until_no_events(
        &mut ctx,
        &action_timeout,
        vec![
            &mut alice,   // 0
            &mut bob,     // 1
            &mut charlie, // 2
            &mut dave,    // 3
            &mut eve,     // 4
            &mut frank,   // 5
            &mut grace,   // 6
            &mut transactor,
        ],
    )?;

    {
        let state = handler.get_state();
        let timeout_player = state.player_map.get(&eve.id()).unwrap();
        assert_eq!(timeout_player.status, PlayerStatus::Fold);
        assert_eq!(timeout_player.timeout, 1);
        assert_eq!(
            state.acting_player,
            Some(ActingPlayer {
                id: frank.id(),
                position: 5,
                clock: 12000,
            })
        );
    }

    let action_timeout = Event::ActionTimeout {
        player_id: frank.id(),
    };
    handler.handle_until_no_events(
        &mut ctx,
        &action_timeout,
        vec![
            &mut alice,   // 0
            &mut bob,     // 1
            &mut charlie, // 2
            &mut dave,    // 3
            &mut eve,     // 4
            &mut frank,   // 5
            &mut grace,   // 6
            &mut transactor,
        ],
    )?;

    {
        let state = handler.get_state();
        let timeout_player = state.player_map.get(&frank.id()).unwrap();
        assert_eq!(timeout_player.status, PlayerStatus::Fold);
        assert_eq!(timeout_player.timeout, 1);
        assert_eq!(
            state.acting_player,
            Some(ActingPlayer {
                id: grace.id(),
                position: 6,
                clock: 12000,
            })
        );
    }

    let action_timeout = Event::ActionTimeout {
        player_id: grace.id(),
    };
    handler.handle_until_no_events(
        &mut ctx,
        &action_timeout,
        vec![
            &mut alice,   // 0
            &mut bob,     // 1
            &mut charlie, // 2
            &mut dave,    // 3
            &mut eve,     // 4
            &mut frank,   // 5
            &mut grace,   // 6
            &mut transactor,
        ],
    )?;

    {
        let state = handler.get_state();
        let timeout_player = state.player_map.get(&grace.id()).unwrap();
        assert_eq!(timeout_player.status, PlayerStatus::Fold);
        assert_eq!(timeout_player.timeout, 1);
    }

    let action_timeout = Event::ActionTimeout {
        player_id: alice.id(),
    };
    handler.handle_until_no_events(
        &mut ctx,
        &action_timeout,
        vec![
            &mut alice,   // 0
            &mut bob,     // 1
            &mut charlie, // 2
            &mut dave,    // 3
            &mut eve,     // 4
            &mut frank,   // 5
            &mut grace,   // 6
            &mut transactor,
        ],
    )?;

    {
        let state = handler.get_state();
        let timeout_player = state.player_map.get(&alice.id()).unwrap();
        assert_eq!(timeout_player.status, PlayerStatus::Fold);
        assert_eq!(timeout_player.timeout, 1);
    }

    let action_timeout = Event::ActionTimeout {
        player_id: bob.id(),
    };
    handler.handle_until_no_events(
        &mut ctx,
        &action_timeout,
        vec![
            &mut alice,   // 0
            &mut bob,     // 1
            &mut charlie, // 2
            &mut dave,    // 3
            &mut eve,     // 4
            &mut frank,   // 5
            &mut grace,   // 6
            &mut transactor,
        ],
    )?;

    {
        let state = handler.get_state();
        let timeout_player = state.player_map.get(&bob.id()).unwrap();
        assert_eq!(timeout_player.status, PlayerStatus::Fold);
        assert_eq!(timeout_player.timeout, 1);
    }

    let action_timeout = Event::ActionTimeout {
        player_id: charlie.id(),
    };
    handler.handle_until_no_events(
        &mut ctx,
        &action_timeout,
        vec![
            &mut alice,   // 0
            &mut bob,     // 1
            &mut charlie, // 2
            &mut dave,    // 3
            &mut eve,     // 4
            &mut frank,   // 5
            &mut grace,   // 6
            &mut transactor,
        ],
    )?;

    {
        let state = handler.get_state();
        let timeout_player = state.player_map.get(&charlie.id()).unwrap();
        assert_eq!(timeout_player.status, PlayerStatus::Fold);
        assert_eq!(timeout_player.timeout, 1);
    }

    {
        let state = handler.get_state();
        let winner = state.player_map.get(&dave.id()).unwrap();
        assert_eq!(winner.status, PlayerStatus::Wait);
        assert_eq!(winner.timeout, 0);
    }

    Ok(())
}
