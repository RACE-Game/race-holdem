mod helper;

use helper::{create_sync_event, setup_holdem_game};
use race_api::{error::Result, event::Event};
use race_holdem_base::essential::*;
use race_test::prelude::*;

// Test one player reaches the maximum number of timeouts in heads up
#[test]
fn test_headsup_action_timeout() -> Result<()> {
    let (_game_acct, mut ctx, mut handler, mut transactor) = setup_holdem_game();
    let mut alice = TestClient::player("Alice");
    let mut bob = TestClient::player("Bob");

    let sync_evt = create_sync_event(&mut ctx, &[&alice, &bob], &transactor);
    handler.handle_until_no_events(
        &mut ctx,
        &sync_evt,
        vec![&mut alice, &mut bob, &mut transactor],
    )?;

    {
        let state = handler.get_mut_state();
        state
            .player_map
            .entry("Bob".to_string())
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
    let (_game_acct, mut ctx, mut handler, mut transactor) = setup_holdem_game();
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
        &[&alice, &bob, &charlie, &dave, &eve, &frank, &grace],
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
                "Eve".to_string(),
                "Frank".to_string(),
                "Grace".to_string(),
                "Alice".to_string(),
                "Bob".to_string(),
                "Charlie".to_string(),
                "Dave".to_string(),
            ]
        );
        assert_eq!(state.street, Street::Preflop);
        assert_eq!(
            state.acting_player,
            Some(ActingPlayer {
                addr: "Eve".to_string(),
                position: 4,
                clock: 15000
            })
        );

        for p in state.player_map.values() {
            println!("Player {} with status {:?}", p.addr, p.status);
        }
     }

    // Let players act timeout one by one
    let action_timeout = Event::ActionTimeout {player_addr: "Eve".to_string()};
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
        let timeout_player = state.player_map.get("Eve").unwrap();
        assert_eq!(timeout_player.status, PlayerStatus::Fold);
        assert_eq!(timeout_player.timeout, 1);
        assert_eq!(
            state.acting_player,
            Some(ActingPlayer {
                addr: "Frank".to_string(),
                position: 5,
                clock: 12000,
            })
        );
    }

    let action_timeout = Event::ActionTimeout {player_addr: "Frank".to_string()};
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
        let timeout_player = state.player_map.get("Frank").unwrap();
        assert_eq!(timeout_player.status, PlayerStatus::Fold);
        assert_eq!(timeout_player.timeout, 1);
        assert_eq!(
            state.acting_player,
            Some(ActingPlayer {
                addr: "Grace".to_string(),
                position: 6,
                clock: 12000,
            })
        );
    }

    let action_timeout = Event::ActionTimeout {player_addr: "Grace".to_string()};
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
        let timeout_player = state.player_map.get("Grace").unwrap();
        assert_eq!(timeout_player.status, PlayerStatus::Fold);
        assert_eq!(timeout_player.timeout, 1);
    }

    let action_timeout = Event::ActionTimeout {player_addr: "Alice".to_string()};
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
        let timeout_player = state.player_map.get("Alice").unwrap();
        assert_eq!(timeout_player.status, PlayerStatus::Fold);
        assert_eq!(timeout_player.timeout, 1);
    }

    let action_timeout = Event::ActionTimeout {player_addr: "Bob".to_string()};
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
        let timeout_player = state.player_map.get("Bob").unwrap();
        assert_eq!(timeout_player.status, PlayerStatus::Fold);
        assert_eq!(timeout_player.timeout, 1);
    }

    let action_timeout = Event::ActionTimeout {player_addr: "Charlie".to_string()};
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
        let timeout_player = state.player_map.get("Charlie").unwrap();
        assert_eq!(timeout_player.status, PlayerStatus::Fold);
        assert_eq!(timeout_player.timeout, 1);
    }

    {
        let state = handler.get_state();
        let winner = state.player_map.get("Dave").unwrap();
        assert_eq!(winner.status, PlayerStatus::Wait);
        assert_eq!(winner.timeout, 0);
    }

    Ok(())
}
