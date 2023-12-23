//! Test Holdem game and its several key aspects such as
//! order of players, runner stage and hole-card dealing.
//! The last test shows a complete hand playing.
//! Note: In real-world games, players join a game one after
//! another, instead of all together as shown in the tests.

mod helper;

use race_api::prelude::*;
use race_test::prelude::*;
use std::collections::HashMap;

use helper::{create_sync_event, setup_holdem_game};
use race_holdem_base::{essential::*, game::Holdem};

#[test]
fn test_players_order() -> Result<()> {
    let (_game_acct, mut ctx, mut handler, mut transactor) = setup_holdem_game();

    let mut alice = TestClient::player("Alice");
    let mut bob = TestClient::player("Bob");
    let mut carol = TestClient::player("Carol");
    let mut dave = TestClient::player("Dave");
    let mut eva = TestClient::player("Eva");

    let sync_evt = create_sync_event(&ctx, &[&alice, &bob, &carol, &dave, &eva], &transactor);

    // ------------------------- GAMESTART ------------------------
    handler.handle_until_no_events(
        &mut ctx,
        &sync_evt,
        vec![
            &mut alice,
            &mut bob,
            &mut carol,
            &mut dave,
            &mut eva,
            &mut transactor,
        ],
    )?;

    {
        let state = handler.get_state();
        // BTN will be 1 so players should be arranged like below:
        assert_eq!(
            state.player_order,
            vec![
                "Eva".to_string(),
                "Alice".to_string(),
                "Bob".to_string(),
                "Carol".to_string(),
                "Dave".to_string(),
            ]
        );
    }

    Ok(())
}

#[test]
fn test_eject_timeout() -> Result<()> {
    let (_game_acct, mut ctx, mut handler, mut transactor) = setup_holdem_game();
    let mut alice = TestClient::player("Alice");
    let mut bob = TestClient::player("Bob");
    let mut charlie = TestClient::player("Charlie");

    let sync_evt = create_sync_event(&ctx, &[&alice, &bob, &charlie], &transactor);
    handler.handle_until_no_events(
        &mut ctx,
        &sync_evt,
        vec![&mut alice, &mut bob, &mut charlie, &mut transactor],
    )?;

    // --------------------- INIT ------------------------
    {
        let state = handler.get_mut_state();
        let bob = state.player_map.get_mut("Bob").unwrap();
        bob.timeout = 3;
        assert_eq!(
            state.player_order,
            vec![
                "Bob".to_string(),     // UTG + BTN
                "Charlie".to_string(), // SB
                "Alice".to_string(),   // BB
            ]
        );

        for p in state.player_map.values() {
            if p.addr == "Alice".to_string() || p.addr == "Charlie".to_string() {
                assert_eq!(p.timeout, 0)
            } else {
                assert_eq!(p.timeout, 3)
            }
        }

        assert_eq!(
            state.acting_player,
            Some(ActingPlayer {
                addr: "Bob".to_string(),
                position: 1,
                clock: ACTION_TIMEOUT_POSTFLOP
            })
        );
    }

    // --------------------- PREFLOP ------------------------
    // Bob (UTG/BTN) reaches action timeout, meets 3 action timeout
    let bob_timeout = Event::ActionTimeout {
        player_addr: "Bob".to_string(),
    };
    handler.handle_until_no_events(
        &mut ctx,
        &bob_timeout,
        vec![&mut alice, &mut bob, &mut charlie, &mut transactor],
    )?;

    {
        let state = handler.get_state();
        assert_eq!(
            state.acting_player,
            Some(ActingPlayer {
                addr: "Charlie".to_string(),
                position: 2,
                clock: ACTION_TIMEOUT_PREFLOP
            })
        );
        for p in state.player_map.values() {
            if p.addr == "Alice".to_string() {
                assert_eq!(p.timeout, 0);
                assert_eq!(p.status, PlayerStatus::Wait);
            } else if p.addr == "Charlie".to_string() {
                assert_eq!(p.status, PlayerStatus::Acting);
                assert_eq!(p.timeout, 0);
            } else {
                assert_eq!(p.status, PlayerStatus::Leave);
                assert_eq!(p.timeout, 3);
            }
        }
    }

    // Charlie (SB) folds, and Alice (BB) wins
    let charlie_fold = charlie.custom_event(GameEvent::Fold);
    handler.handle_until_no_events(
        &mut ctx,
        &charlie_fold,
        vec![&mut charlie, &mut alice, &mut bob, &mut transactor],
    )?;

    {
        let state = handler.get_state();
        assert_eq!(state.player_map.len(), 2);
        assert!(state.player_map.contains_key(&"Alice".to_string()));
        assert!(state.player_map.contains_key(&"Charlie".to_string()));
    }

    Ok(())
}

#[test]
fn test_eject_loser() -> Result<()> {
    let (_game_acct, mut ctx, mut handler, mut transactor) = setup_holdem_game();
    let mut alice = TestClient::player("Alice");
    let mut bob = TestClient::player("Bob");
    let mut charlie = TestClient::player("Charlie");

    let sync_evt = create_sync_event(&ctx, &[&alice, &bob, &charlie], &transactor);
    handler.handle_until_no_events(
        &mut ctx,
        &sync_evt,
        vec![&mut alice, &mut bob, &mut charlie, &mut transactor],
    )?;
    {
        let state = handler.get_state();
        assert_eq!(state.street, Street::Preflop);
        assert_eq!(state.stage, HoldemStage::Play);
        assert_eq!(
            state.player_order,
            vec![
                "Bob".to_string(),     // BTN
                "Charlie".to_string(), // SB
                "Alice".to_string(),   // BB
            ]
        );
        assert_eq!(
            state.acting_player,
            Some(ActingPlayer {
                addr: "Bob".to_string(),
                position: 1,
                clock: ACTION_TIMEOUT_POSTFLOP
            })
        );
    }

    // Pin the settle result for test purposes
    let runner_revealed = HashMap::from([
        // Alice
        (0, "st".to_string()),
        (1, "ct".to_string()),
        // Bob
        (2, "ht".to_string()),
        (3, "dq".to_string()),
        // Charlie
        (4, "s2".to_string()),
        (5, "d5".to_string()),
        // Board
        (6, "s5".to_string()),
        (7, "c6".to_string()),
        (8, "h2".to_string()),
        (9, "h8".to_string()),
        (10, "d7".to_string()),
    ]);
    let holdem_state = handler.get_state();
    ctx.add_revealed_random(holdem_state.deck_random_id, runner_revealed)?;
    println!(
        "-- Cards {:?}",
        ctx.get_revealed(holdem_state.deck_random_id)?
    );

    // BTN goes all in
    let bob_allin = bob.custom_event(GameEvent::Raise(10_000));
    handler.handle_until_no_events(
        &mut ctx,
        &bob_allin,
        vec![&mut alice, &mut bob, &mut charlie, &mut transactor],
    )?;

    {
        let state = handler.get_state();
        assert_eq!(
            state.player_map.get("Bob").unwrap().status,
            PlayerStatus::Allin
        );
        assert_eq!(state.street_bet, 10_000);
        assert_eq!(state.min_raise, 9980);
        assert_eq!(
            state.acting_player,
            Some(ActingPlayer {
                addr: "Charlie".to_string(),
                position: 2,
                clock: ACTION_TIMEOUT_POSTFLOP
            })
        );
    }

    // SB folds
    let charlie_fold = charlie.custom_event(GameEvent::Fold);
    handler.handle_until_no_events(
        &mut ctx,
        &charlie_fold,
        vec![&mut alice, &mut bob, &mut charlie, &mut transactor],
    )?;
    {
        let state = handler.get_state();
        assert_eq!(
            state.player_map.get("Charlie").unwrap().status,
            PlayerStatus::Fold
        );
        assert_eq!(
            state.acting_player,
            Some(ActingPlayer {
                addr: "Alice".to_string(),
                position: 0,
                clock: ACTION_TIMEOUT_POSTFLOP
            })
        );
    }

    // BB chooses to make a hero call
    let alice_allin = alice.custom_event(GameEvent::Call);
    handler.handle_until_no_events(
        &mut ctx,
        &alice_allin,
        vec![&mut alice, &mut bob, &mut charlie, &mut transactor],
    )?;

    // Game enters Runner stage and Alice wins
    // NOTE: At this moment, chips of winner = chips lost by others - chips winner bet
    {
        let state = handler.get_state();
        assert_eq!(state.stage, HoldemStage::Runner);
        assert_eq!(state.street, Street::Showdown);
        assert_eq!(state.street_bet, 0);
        assert_eq!(state.min_raise, 0);
        assert_eq!(state.player_map.len(), 2);
        assert_eq!(state.acting_player, None);
        for player in state.player_map.values() {
            if player.addr == "Charlie".to_string() {
                assert_eq!(player.status, PlayerStatus::Fold);
                assert_eq!(player.chips, 9990);
            } else if player.addr == "Bob".to_string() {
                assert_eq!(player.status, PlayerStatus::Out);
                assert_eq!(player.chips, 0);
            } else {
                assert_eq!(player.status, PlayerStatus::Allin);
                // At this moment, the 20 Alice bet has not been returned yet
                assert_eq!(player.chips, 19_950);
            }
        }
    }

    // Handle the Waitimeout Event
    handler.handle_dispatch_event(&mut ctx)?;

    // Game should start again and Bob gets marked as out
    {
        let state = handler.get_state();
        assert_eq!(state.player_map.len(), 2);
        assert!(state
            .player_map
            .iter()
            .all(|(_, p)| p.status == PlayerStatus::Wait));
        assert_eq!(state.stage, HoldemStage::Init);
        assert_eq!(state.street, Street::Init);
    }
    Ok(())
}

#[test]
fn test_get_holecards_idxs() -> Result<()> {
    let (_game_acct, mut ctx, mut handler, mut transactor) = setup_holdem_game();
    let mut alice = TestClient::player("Alice");
    let mut bob = TestClient::player("Bob");
    let sync_evt = create_sync_event(&ctx, &[&alice, &bob], &transactor);
    // Syncing players to the game, i.e. they join the game and game kicks start
    handler.handle_until_no_events(
        &mut ctx,
        &sync_evt,
        vec![&mut alice, &mut bob, &mut transactor],
    )?;

    let holdem_state = handler.get_state();
    {
        println!("-- Player hand index map {:?}", holdem_state.hand_index_map);
        let alice_hole_cards = alice.decrypt(&ctx, holdem_state.deck_random_id);
        println!("-- Alice hole cards {:?}", alice_hole_cards);

        let alice_hand_index = holdem_state
            .hand_index_map
            .get(&"Alice".to_string())
            .unwrap();
        assert_eq!(alice_hand_index, &vec![0, 1]);
    }
    Ok(())
}

#[test]
fn test_runner() -> Result<()> {
    let (_game_acct, mut ctx, mut handler, mut transactor) = setup_holdem_game();
    let mut alice = TestClient::player("Alice");
    let mut bob = TestClient::player("Bob");
    let sync_evt = create_sync_event(&ctx, &[&alice, &bob], &transactor);

    // Syncing players to the game, i.e. they join the game and game kicks start
    handler.handle_until_no_events(
        &mut ctx,
        &sync_evt,
        vec![&mut alice, &mut bob, &mut transactor],
    )?;

    let runner_revealed = HashMap::from([
        // Alice
        (0, "st".to_string()),
        (1, "ct".to_string()),
        // Bob
        (2, "ht".to_string()),
        (3, "dt".to_string()),
        // Board
        (4, "s5".to_string()),
        (5, "c6".to_string()),
        (6, "h2".to_string()),
        (7, "h8".to_string()),
        (8, "d7".to_string()),
    ]);
    let holdem_state = handler.get_state();
    ctx.add_revealed_random(holdem_state.deck_random_id, runner_revealed)?;
    println!(
        "-- Cards {:?}",
        ctx.get_revealed(holdem_state.deck_random_id)?
    );

    // With everything ready, game enters preflop
    {
        assert_eq!(
            RandomStatus::Ready,
            ctx.get_random_state_unchecked(1).status
        );

        let state = handler.get_state();
        assert_eq!(state.street, Street::Preflop,);
        assert_eq!(ctx.count_players(), 2);
        assert_eq!(ctx.get_status(), GameStatus::Running);
        assert_eq!(
            *ctx.get_dispatch(),
            Some(DispatchEvent {
                timeout: ACTION_TIMEOUT_POSTFLOP,
                event: Event::ActionTimeout {
                    player_addr: "Bob".into()
                },
            })
        );
        assert!(state.is_acting_player(&"Bob".to_string()));
        assert_eq!(
            state.acting_player,
            Some(ActingPlayer {
                addr: "Bob".to_string(),
                position: 1,
                clock: ACTION_TIMEOUT_POSTFLOP,
            })
        );
    }

    // ------------------------- PREFLOP ------------------------
    // Bob(BTN) decides to go all in
    let bob_allin = bob.custom_event(GameEvent::Raise(9990));
    handler.handle_until_no_events(
        &mut ctx,
        &bob_allin,
        vec![&mut alice, &mut bob, &mut transactor],
    )?;

    // Alice(BB) decides to make a hero call
    let alice_call = alice.custom_event(GameEvent::Call);
    handler.handle_until_no_events(
        &mut ctx,
        &alice_call,
        vec![&mut alice, &mut bob, &mut transactor],
    )?;

    // ------------------------- RUNNER ------------------------
    {
        let state = handler.get_state();
        assert_eq!(state.pots.len(), 1);
        assert_eq!(state.pots[0].owners.len(), 2);
        assert_eq!(state.pots[0].winners.len(), 2); // a draw

        let alice = state.player_map.get("Alice").unwrap();
        let bob = state.player_map.get("Bob").unwrap();
        assert_eq!(alice.status, PlayerStatus::Allin);
        assert_eq!(bob.status, PlayerStatus::Allin);

        println!("-- Display {:?}", state.display);
        assert_eq!(state.board.len(), 5);
        assert!(state.display.contains(&Display::DealBoard {
            prev: 0,
            board: vec![
                "s5".to_string(),
                "c6".to_string(),
                "h2".to_string(),
                "h8".to_string(),
                "d7".to_string(),
            ]
        }));
        assert!(state.display.contains(&Display::AwardPots {
            pots: vec![AwardPot {
                winners: vec!["Bob".to_string(), "Alice".to_string()],
                amount: 20000
            }]
        }))
    }

    Ok(())
}

#[test]
fn test_settle_stage() -> Result<()> {
    let (_game_acct, mut ctx, mut handler, mut transactor) = setup_holdem_game();
    let mut alice = TestClient::player("Alice");
    let mut bob = TestClient::player("Bob");
    let mut charlie = TestClient::player("Charlie");
    let sync_evt = create_sync_event(&ctx, &[&alice, &bob, &charlie], &transactor);

    // Syncing players to the game, i.e. they join the game and game kicks start
    handler.handle_until_no_events(
        &mut ctx,
        &sync_evt,
        vec![&mut alice, &mut bob, &mut charlie, &mut transactor],
    )?;

    {
        let state = handler.get_state();
        assert_eq!(state.street, Street::Preflop);
        assert_eq!(state.stage, HoldemStage::Play);
        assert_eq!(state.street_bet, 20);
        assert_eq!(state.min_raise, 20);
        assert_eq!(
            state.player_order,
            vec![
                "Bob".to_string(),     // BTN
                "Charlie".to_string(), // SB
                "Alice".to_string(),   // BB
            ]
        );
        assert_eq!(
            state.acting_player,
            Some(ActingPlayer {
                addr: "Bob".to_string(),
                position: 1,
                clock: ACTION_TIMEOUT_POSTFLOP
            })
        );
    }

    // BTN and SB all decide to fold so BB (single player) wins
    let bob_fold = bob.custom_event(GameEvent::Fold);
    handler.handle_until_no_events(
        &mut ctx,
        &bob_fold,
        vec![&mut alice, &mut bob, &mut charlie, &mut transactor],
    )?;

    {
        let state = handler.get_state();
        assert_eq!(
            state.acting_player,
            Some(ActingPlayer {
                addr: "Charlie".to_string(),
                position: 2,
                clock: ACTION_TIMEOUT_PREFLOP
            })
        );
        assert_eq!(state.street_bet, 20);
        assert_eq!(state.min_raise, 20);
    }

    let charlie_fold = charlie.custom_event(GameEvent::Fold);
    handler.handle_until_no_events(
        &mut ctx,
        &charlie_fold,
        vec![&mut alice, &mut bob, &mut charlie, &mut transactor],
    )?;

    // Game then should enter into `Settle` stage
    {
        let state = handler.get_state();
        assert_eq!(state.street, Street::Preflop);
        assert_eq!(state.stage, HoldemStage::Settle);
        assert_eq!(state.street_bet, 0);
        assert_eq!(state.min_raise, 0);
        assert_eq!(state.acting_player, None);
        for player in state.player_map.values() {
            if player.addr == "Charlie".to_string() || player.addr == "Bob".to_string() {
                assert!(matches!(player.status, PlayerStatus::Fold));
            } else {
                assert_eq!(player.status, PlayerStatus::Wait);
            }
        }
        assert_eq!(state.winners, vec!["Alice".to_string()]);
    }

    Ok(())
}

#[test]
#[ignore]
// For debugging purposes only
fn test_abnormal_street() -> Result<()> {
    let (_game_acct, mut ctx, mut handler, mut transactor) = setup_holdem_game();
    let mut alice = TestClient::player("Alice");
    let mut bob = TestClient::player("Bob");
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
            vec!["Alice".to_string(), "Bob".to_string(),]
        );
        assert_eq!(
            state.acting_player,
            Some(ActingPlayer {
                addr: "Alice".to_string(),
                position: 0,
                clock: 30_000
            })
        );
    }
    // SB calls and BB checks --> Flop
    let sb_call = alice.custom_event(GameEvent::Call);
    handler.handle_until_no_events(
        &mut ctx,
        &sb_call,
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
        println!("-- Bet map {:?}", state.bet_map);
        assert_eq!(state.street, Street::Flop);
        assert_eq!(
            state.acting_player,
            Some(ActingPlayer {
                addr: "Alice".to_string(),
                position: 0,
                clock: 30_000
            })
        );
    }

    // To reproduce the bug, in flop, player A checks and player B bets,
    // Expected: game remains in flop and player A is asked to act
    // Got: game enters turn
    let sb_check = alice.custom_event(GameEvent::Check);
    handler.handle_until_no_events(
        &mut ctx,
        &sb_check,
        vec![&mut alice, &mut bob, &mut transactor],
    )?;

    let bb_bet = bob.custom_event(GameEvent::Bet(40));
    handler.handle_until_no_events(
        &mut ctx,
        &bb_bet,
        vec![&mut alice, &mut bob, &mut transactor],
    )?;

    {
        let state = handler.get_state();
        assert_eq!(state.player_map.get("Bob").unwrap().chips, 9940);
        println!("-- State {:?}", state);
        assert_eq!(state.street, Street::Flop);
    }

    Ok(())
}

#[test]
fn test_play_game() -> Result<()> {
    let (_game_acct, mut ctx, mut handler, mut transactor) = setup_holdem_game();
    let mut alice = TestClient::player("Alice");
    let mut bob = TestClient::player("Bob");
    let mut carol = TestClient::player("Carol");
    let mut dave = TestClient::player("Dave");
    let mut eva = TestClient::player("Eva");

    let sync_evt = create_sync_event(&ctx, &[&alice, &bob, &carol, &dave, &eva], &transactor);

    // ------------------------- GAMESTART ------------------------
    handler.handle_until_no_events(
        &mut ctx,
        &sync_evt,
        vec![
            &mut alice,
            &mut bob,
            &mut carol,
            &mut dave,
            &mut eva,
            &mut transactor,
        ],
    )?;

    // After game starts, a random state will be initialized and ready for game
    // In this stage, players are assigned/dealt folded cards
    // Pin the randomness for testing purposes
    let revealed = HashMap::from([
        // Alice
        (0, "c9".to_string()),
        (1, "d3".to_string()),
        // Bob
        (2, "ht".to_string()),
        (3, "d8".to_string()),
        // Carol
        (4, "st".to_string()),
        (5, "ct".to_string()),
        // Dave
        (6, "sq".to_string()),
        (7, "d2".to_string()),
        // Eva
        (8, "h3".to_string()),
        (9, "dk".to_string()),
        // Board
        (10, "s5".to_string()),
        (11, "c6".to_string()),
        (12, "h2".to_string()),
        (13, "h8".to_string()),
        (14, "d7".to_string()),
    ]);
    let holdem_state = handler.get_state();
    ctx.add_revealed_random(holdem_state.deck_random_id, revealed)?;
    println!(
        "-- Cards {:?}",
        ctx.get_revealed(holdem_state.deck_random_id)?
    );

    // ------------------------- BLIND BETS ----------------------
    {
        // BTN will be 1 so players in the order of action:
        // Eva (UTG), Alice (CO), Bob (BTN), Carol (SB), Dave (BB)
        // In state.player_order: [Carol, Dave, Eva, Alice, Bob]

        // UTG decides to leave
        println!("context {:?}", ctx);
        let eva_leave = Event::Leave {
            player_addr: "Eva".to_string(),
        };
        handler.handle_until_no_events(
            &mut ctx,
            &eva_leave,
            vec![
                &mut alice,
                &mut bob,
                &mut carol,
                &mut dave,
                &mut eva,
                &mut transactor,
            ],
        )?;

        // CO calls
        let alice_call = alice.custom_event(GameEvent::Call);
        handler.handle_until_no_events(
            &mut ctx,
            &alice_call,
            vec![
                &mut alice,
                &mut bob,
                &mut carol,
                &mut dave,
                &mut eva,
                &mut transactor,
            ],
        )?;

        // BTN calls
        let bob_call = bob.custom_event(GameEvent::Call);
        handler.handle_until_no_events(
            &mut ctx,
            &bob_call,
            vec![
                &mut alice,
                &mut bob,
                &mut carol,
                &mut dave,
                &mut eva,
                &mut transactor,
            ],
        )?;

        // SB calls
        let carol_call = carol.custom_event(GameEvent::Call);
        handler.handle_until_no_events(
            &mut ctx,
            &carol_call,
            vec![
                &mut alice,
                &mut bob,
                &mut carol,
                &mut dave,
                &mut eva,
                &mut transactor,
            ],
        )?;

        {
            let state = handler.get_state();

            for p in state.player_map.values() {
                println!("-- Player {} position {}", p.addr, p.position);
            }

            assert_eq!(state.street, Street::Preflop,);
            assert_eq!(
                state.player_map.get(&"Eva".to_string()).unwrap().status,
                PlayerStatus::Leave
            );
            // Acting player is the next player, BB, Dave
            assert!(state.acting_player.is_some());
            assert_eq!(
                state.acting_player,
                Some(ActingPlayer {
                    addr: "Dave".to_string(),
                    position: 3usize,
                    clock: 12_000u64
                })
            );
        }

        // BB checks then game goes to flop
        let dave_check = dave.custom_event(GameEvent::Check);
        handler.handle_until_no_events(
            &mut ctx,
            &dave_check,
            vec![
                &mut alice,
                &mut bob,
                &mut carol,
                &mut dave,
                &mut eva,
                &mut transactor,
            ],
        )?;
    }

    // ------------------------- THE FLOP ------------------------
    // Remained players: Carol (SB), Dave (BB), Alice (CO), Bob (BTN)
    {
        // Test pots
        {
            let state = handler.get_state();
            assert_eq!(state.street, Street::Flop);
            assert_eq!(state.street_bet, 0);
            assert_eq!(state.min_raise, 20);
            assert_eq!(state.pots.len(), 1);
            assert_eq!(state.pots[0].amount, 80);
            assert_eq!(state.pots[0].owners.len(), 4);
            println!("-- Display {:?}", state.display);
            assert!(state.display.contains(&Display::DealBoard {
                prev: 0,
                board: vec!["s5".to_string(), "c6".to_string(), "h2".to_string(),]
            }));
        }

        // Frank Joins:
        // 1. Frank's status should be `Init`
        // 2. Frank should be in player_map but not in player_order
        // 3. Frank should not be assgined any cards, i.e., not in hand_index_map
        let mut frank = TestClient::player("Frank");
        let frank_join = create_sync_event(&ctx, &[&frank], &transactor);

        handler.handle_until_no_events(
            &mut ctx,
            &frank_join,
            vec![
                &mut alice,
                &mut bob,
                &mut carol,
                &mut dave,
                &mut eva,
                &mut frank,
                &mut transactor,
            ],
        )?;
        {
            let state = handler.get_state();
            assert_eq!(state.player_map.len(), 6);
            assert_eq!(state.player_order.len(), 5);
            assert!(matches!(
                state.player_map.get("Frank").unwrap().status,
                PlayerStatus::Init
            ));
            assert_eq!(state.hand_index_map.get("Frank"), None);
            assert_eq!(
                state.acting_player.as_ref().unwrap().addr,
                "Carol".to_string()
            );
        }

        // Carol(SB) bets 1BB
        let carol_bet = carol.custom_event(GameEvent::Bet(20));
        handler.handle_until_no_events(
            &mut ctx,
            &carol_bet,
            vec![
                &mut alice,
                &mut bob,
                &mut carol,
                &mut dave,
                &mut eva,
                &mut transactor,
            ],
        )?;

        {
            let state = handler.get_state();
            assert_eq!(state.street_bet, 20);
        }

        // Dave(BB) calls
        let dave_call = dave.custom_event(GameEvent::Call);
        handler.handle_until_no_events(
            &mut ctx,
            &dave_call,
            vec![
                &mut alice,
                &mut bob,
                &mut carol,
                &mut dave,
                &mut eva,
                &mut transactor,
            ],
        )?;

        // Alice(CO) calls
        let alice_call = alice.custom_event(GameEvent::Call);
        handler.handle_until_no_events(
            &mut ctx,
            &alice_call,
            vec![
                &mut alice,
                &mut bob,
                &mut carol,
                &mut dave,
                &mut eva,
                &mut transactor,
            ],
        )?;

        // Bob(BTN) calls
        let bob_call = bob.custom_event(GameEvent::Call);
        handler.handle_until_no_events(
            &mut ctx,
            &bob_call,
            vec![
                &mut alice,
                &mut bob,
                &mut carol,
                &mut dave,
                &mut eva,
                &mut transactor,
            ],
        )?;
    }

    // ------------------------- THE TURN ------------------------
    // Remained players: Carol (SB), Dave (BB), Alice (CO), Bob (BTN)
    {
        {
            // Test pots
            let state = handler.get_state();
            assert_eq!(state.street, Street::Turn);
            assert_eq!(state.street_bet, 0);
            assert_eq!(state.min_raise, 20);
            assert_eq!(state.pots.len(), 1);
            assert_eq!(state.pots[0].amount, 160);
            assert_eq!(state.pots[0].owners.len(), 4);
            assert!(state.pots[0].owners.contains(&"Alice".to_string()));
            assert!(state.pots[0].owners.contains(&"Bob".to_string()));
            assert!(state.pots[0].owners.contains(&"Carol".to_string()));
            assert!(state.pots[0].owners.contains(&"Dave".to_string()));
            assert_eq!(
                state.board,
                vec![
                    "s5".to_string(),
                    "c6".to_string(),
                    "h2".to_string(),
                    "h8".to_string(),
                ]
            );
            assert!(state.display.contains(&Display::DealBoard {
                prev: 3,
                board: vec![
                    "s5".to_string(),
                    "c6".to_string(),
                    "h2".to_string(),
                    "h8".to_string(),
                ]
            }));
        }

        // Carol (SB) decides to c-bet 1BB
        let carol_bet = carol.custom_event(GameEvent::Bet(20));
        handler.handle_until_no_events(
            &mut ctx,
            &carol_bet,
            vec![
                &mut alice,
                &mut bob,
                &mut carol,
                &mut dave,
                &mut eva,
                &mut transactor,
            ],
        )?;

        {
            // Test min raise
            let state = handler.get_state();
            assert_eq!(20, state.street_bet);
            assert_eq!(20, state.min_raise);
        }

        // Dave (BB) decides to raise
        let dave_raise = dave.custom_event(GameEvent::Raise(60));
        handler.handle_until_no_events(
            &mut ctx,
            &dave_raise,
            vec![
                &mut alice,
                &mut bob,
                &mut carol,
                &mut dave,
                &mut eva,
                &mut transactor,
            ],
        )?;

        {
            // Test min raise
            let state = handler.get_state();
            assert_eq!(state.street_bet, 60);
            assert_eq!(state.min_raise, 40);
        }

        // Alice(SB) folds
        let alice_fold = alice.custom_event(GameEvent::Fold);
        handler.handle_until_no_events(
            &mut ctx,
            &alice_fold,
            vec![
                &mut alice,
                &mut bob,
                &mut carol,
                &mut dave,
                &mut eva,
                &mut transactor,
            ],
        )?;

        // Bob (BTN) calls
        let bob_call = bob.custom_event(GameEvent::Call);
        handler.handle_until_no_events(
            &mut ctx,
            &bob_call,
            vec![
                &mut alice,
                &mut bob,
                &mut carol,
                &mut dave,
                &mut eva,
                &mut transactor,
            ],
        )?;

        // Carol (SB) calls
        let carol_call = carol.custom_event(GameEvent::Call);
        handler.handle_until_no_events(
            &mut ctx,
            &carol_call,
            vec![
                &mut alice,
                &mut bob,
                &mut carol,
                &mut dave,
                &mut eva,
                &mut transactor,
            ],
        )?;
    }

    // ------------------------- THE RIVER ------------------------
    // Remained players: Carol (SB), Dave (BB), Bob (BTN)
    {
        {
            // Test pots
            let state = handler.get_state();
            assert_eq!(state.street, Street::River);
            assert_eq!(state.street_bet, 0);
            assert_eq!(state.min_raise, 20);
            assert_eq!(state.pots.len(), 1);
            // Pot 1
            assert_eq!(state.pots[0].amount, 340);
            assert_eq!(state.pots[0].owners.len(), 3);
            assert!(state.pots[0].owners.contains(&"Bob".to_string()));
            assert!(state.pots[0].owners.contains(&"Carol".to_string()));
            assert!(state.pots[0].owners.contains(&"Dave".to_string()));
            // Board and display
            assert_eq!(
                state.board,
                vec![
                    "s5".to_string(),
                    "c6".to_string(),
                    "h2".to_string(),
                    "h8".to_string(),
                    "d7".to_string(),
                ]
            );
            assert!(state.display.contains(&Display::DealBoard {
                prev: 4,
                board: vec![
                    "s5".to_string(),
                    "c6".to_string(),
                    "h2".to_string(),
                    "h8".to_string(),
                    "d7".to_string(),
                ]
            }));
        }

        // Carol continues to bet
        let carol_bet = carol.custom_event(GameEvent::Bet(40));
        handler.handle_until_no_events(
            &mut ctx,
            &carol_bet,
            vec![
                &mut alice,
                &mut bob,
                &mut carol,
                &mut dave,
                &mut eva,
                &mut transactor,
            ],
        )?;

        // Dave calls
        let dave_call = dave.custom_event(GameEvent::Call);
        handler.handle_until_no_events(
            &mut ctx,
            &dave_call,
            vec![
                &mut alice,
                &mut bob,
                &mut carol,
                &mut dave,
                &mut eva,
                &mut transactor,
            ],
        )?;

        // Bob calls so it's showdown time
        let bob_call = bob.custom_event(GameEvent::Call);
        handler.handle_until_no_events(
            &mut ctx,
            &bob_call,
            vec![
                &mut alice,
                &mut bob,
                &mut carol,
                &mut dave,
                &mut eva,
                &mut transactor,
            ],
        )?;

        // Wait for 10 secs and game should start again
        handler.handle_dispatch_event(&mut ctx)?;
        {
            let state = handler.get_state();
            assert_eq!(state.btn, 1);
            assert_eq!(state.player_map.len(), 5);
            // Player order has not been cleared yet
            assert_eq!(state.player_order.len(), 0);
        }

        // Handle GameStart
        handler.handle_dispatch_event(&mut ctx)?;
        {
            let state = handler.get_state();
            assert_eq!(state.player_map.len(), 5);
            assert_eq!(state.player_order.len(), 0);
            assert_eq!(state.hand_index_map.len(), 0);
        }
    }
    Ok(())
}

#[test]
fn test_3() -> Result<()> {
    let data = [
        1, 0, 0, 0, 0, 0, 0, 0, 168, 97, 0, 0, 0, 0, 0, 0, 80, 195, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 3, 5, 0, 0, 0, 0, 0, 0, 0, 0, 5, 0, 0, 0, 2, 0,
        0, 0, 104, 53, 2, 0, 0, 0, 100, 52, 2, 0, 0, 0, 104, 55, 2, 0, 0, 0, 104, 54, 2, 0, 0, 0,
        104, 56, 4, 0, 0, 0, 44, 0, 0, 0, 66, 55, 77, 121, 51, 101, 110, 72, 83, 87, 121, 87, 84,
        106, 89, 105, 52, 69, 120, 54, 107, 104, 55, 74, 69, 89, 69, 115, 69, 57, 70, 111, 80, 82,
        51, 53, 115, 49, 53, 117, 87, 67, 65, 102, 2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0,
        0, 0, 0, 0, 44, 0, 0, 0, 66, 114, 105, 53, 119, 122, 122, 86, 121, 66, 75, 52, 105, 54,
        121, 50, 89, 121, 69, 84, 57, 106, 87, 114, 98, 117, 67, 50, 88, 51, 69, 67, 99, 72, 100,
        66, 119, 54, 69, 49, 100, 50, 71, 110, 2, 0, 0, 0, 2, 0, 0, 0, 0, 0, 0, 0, 3, 0, 0, 0, 0,
        0, 0, 0, 44, 0, 0, 0, 69, 78, 82, 49, 49, 118, 80, 78, 119, 50, 120, 80, 88, 107, 67, 74,
        107, 49, 87, 111, 104, 101, 117, 105, 49, 89, 114, 100, 54, 56, 78, 71, 87, 68, 66, 109,
        103, 119, 83, 100, 122, 99, 119, 80, 2, 0, 0, 0, 4, 0, 0, 0, 0, 0, 0, 0, 5, 0, 0, 0, 0, 0,
        0, 0, 43, 0, 0, 0, 106, 111, 105, 90, 90, 118, 105, 109, 68, 52, 76, 80, 66, 55, 100, 72,
        67, 106, 116, 119, 71, 88, 86, 110, 82, 66, 118, 117, 80, 54, 49, 67, 54, 105, 65, 87, 101,
        89, 109, 78, 87, 104, 80, 2, 0, 0, 0, 6, 0, 0, 0, 0, 0, 0, 0, 7, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0, 4, 0, 0, 0, 44, 0, 0, 0, 66, 55, 77, 121, 51, 101, 110, 72, 83, 87, 121, 87, 84, 106,
        89, 105, 52, 69, 120, 54, 107, 104, 55, 74, 69, 89, 69, 115, 69, 57, 70, 111, 80, 82, 51,
        53, 115, 49, 53, 117, 87, 67, 65, 102, 40, 142, 128, 0, 0, 0, 0, 0, 44, 0, 0, 0, 66, 114,
        105, 53, 119, 122, 122, 86, 121, 66, 75, 52, 105, 54, 121, 50, 89, 121, 69, 84, 57, 106,
        87, 114, 98, 117, 67, 50, 88, 51, 69, 67, 99, 72, 100, 66, 119, 54, 69, 49, 100, 50, 71,
        110, 48, 229, 17, 1, 0, 0, 0, 0, 44, 0, 0, 0, 69, 78, 82, 49, 49, 118, 80, 78, 119, 50,
        120, 80, 88, 107, 67, 74, 107, 49, 87, 111, 104, 101, 117, 105, 49, 89, 114, 100, 54, 56,
        78, 71, 87, 68, 66, 109, 103, 119, 83, 100, 122, 99, 119, 80, 240, 73, 2, 0, 0, 0, 0, 0,
        43, 0, 0, 0, 106, 111, 105, 90, 90, 118, 105, 109, 68, 52, 76, 80, 66, 55, 100, 72, 67,
        106, 116, 119, 71, 88, 86, 110, 82, 66, 118, 117, 80, 54, 49, 67, 54, 105, 65, 87, 101, 89,
        109, 78, 87, 104, 80, 48, 229, 17, 1, 0, 0, 0, 0, 0, 0, 0, 0, 4, 0, 0, 0, 44, 0, 0, 0, 66,
        55, 77, 121, 51, 101, 110, 72, 83, 87, 121, 87, 84, 106, 89, 105, 52, 69, 120, 54, 107,
        104, 55, 74, 69, 89, 69, 115, 69, 57, 70, 111, 80, 82, 51, 53, 115, 49, 53, 117, 87, 67,
        65, 102, 44, 0, 0, 0, 66, 55, 77, 121, 51, 101, 110, 72, 83, 87, 121, 87, 84, 106, 89, 105,
        52, 69, 120, 54, 107, 104, 55, 74, 69, 89, 69, 115, 69, 57, 70, 111, 80, 82, 51, 53, 115,
        49, 53, 117, 87, 67, 65, 102, 0, 0, 0, 0, 0, 0, 0, 0, 4, 0, 0, 0, 0, 0, 0, 0, 3, 0, 44, 0,
        0, 0, 66, 114, 105, 53, 119, 122, 122, 86, 121, 66, 75, 52, 105, 54, 121, 50, 89, 121, 69,
        84, 57, 106, 87, 114, 98, 117, 67, 50, 88, 51, 69, 67, 99, 72, 100, 66, 119, 54, 69, 49,
        100, 50, 71, 110, 44, 0, 0, 0, 66, 114, 105, 53, 119, 122, 122, 86, 121, 66, 75, 52, 105,
        54, 121, 50, 89, 121, 69, 84, 57, 106, 87, 114, 98, 117, 67, 50, 88, 51, 69, 67, 99, 72,
        100, 66, 119, 54, 69, 49, 100, 50, 71, 110, 0, 0, 0, 0, 0, 0, 0, 0, 3, 0, 0, 0, 0, 0, 0, 0,
        3, 0, 44, 0, 0, 0, 69, 78, 82, 49, 49, 118, 80, 78, 119, 50, 120, 80, 88, 107, 67, 74, 107,
        49, 87, 111, 104, 101, 117, 105, 49, 89, 114, 100, 54, 56, 78, 71, 87, 68, 66, 109, 103,
        119, 83, 100, 122, 99, 119, 80, 44, 0, 0, 0, 69, 78, 82, 49, 49, 118, 80, 78, 119, 50, 120,
        80, 88, 107, 67, 74, 107, 49, 87, 111, 104, 101, 117, 105, 49, 89, 114, 100, 54, 56, 78,
        71, 87, 68, 66, 109, 103, 119, 83, 100, 122, 99, 119, 80, 224, 15, 151, 0, 0, 0, 0, 0, 0,
        0, 0, 0, 0, 0, 0, 0, 4, 0, 43, 0, 0, 0, 106, 111, 105, 90, 90, 118, 105, 109, 68, 52, 76,
        80, 66, 55, 100, 72, 67, 106, 116, 119, 71, 88, 86, 110, 82, 66, 118, 117, 80, 54, 49, 67,
        54, 105, 65, 87, 101, 89, 109, 78, 87, 104, 80, 43, 0, 0, 0, 106, 111, 105, 90, 90, 118,
        105, 109, 68, 52, 76, 80, 66, 55, 100, 72, 67, 106, 116, 119, 71, 88, 86, 110, 82, 66, 118,
        117, 80, 54, 49, 67, 54, 105, 65, 87, 101, 89, 109, 78, 87, 104, 80, 64, 181, 100, 0, 0, 0,
        0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 3, 0, 4, 0, 0, 0, 44, 0, 0, 0, 66, 55, 77, 121, 51, 101, 110,
        72, 83, 87, 121, 87, 84, 106, 89, 105, 52, 69, 120, 54, 107, 104, 55, 74, 69, 89, 69, 115,
        69, 57, 70, 111, 80, 82, 51, 53, 115, 49, 53, 117, 87, 67, 65, 102, 44, 0, 0, 0, 69, 78,
        82, 49, 49, 118, 80, 78, 119, 50, 120, 80, 88, 107, 67, 74, 107, 49, 87, 111, 104, 101,
        117, 105, 49, 89, 114, 100, 54, 56, 78, 71, 87, 68, 66, 109, 103, 119, 83, 100, 122, 99,
        119, 80, 43, 0, 0, 0, 106, 111, 105, 90, 90, 118, 105, 109, 68, 52, 76, 80, 66, 55, 100,
        72, 67, 106, 116, 119, 71, 88, 86, 110, 82, 66, 118, 117, 80, 54, 49, 67, 54, 105, 65, 87,
        101, 89, 109, 78, 87, 104, 80, 44, 0, 0, 0, 66, 114, 105, 53, 119, 122, 122, 86, 121, 66,
        75, 52, 105, 54, 121, 50, 89, 121, 69, 84, 57, 106, 87, 114, 98, 117, 67, 50, 88, 51, 69,
        67, 99, 72, 100, 66, 119, 54, 69, 49, 100, 50, 71, 110, 2, 0, 0, 0, 3, 0, 0, 0, 44, 0, 0,
        0, 66, 55, 77, 121, 51, 101, 110, 72, 83, 87, 121, 87, 84, 106, 89, 105, 52, 69, 120, 54,
        107, 104, 55, 74, 69, 89, 69, 115, 69, 57, 70, 111, 80, 82, 51, 53, 115, 49, 53, 117, 87,
        67, 65, 102, 44, 0, 0, 0, 66, 114, 105, 53, 119, 122, 122, 86, 121, 66, 75, 52, 105, 54,
        121, 50, 89, 121, 69, 84, 57, 106, 87, 114, 98, 117, 67, 50, 88, 51, 69, 67, 99, 72, 100,
        66, 119, 54, 69, 49, 100, 50, 71, 110, 43, 0, 0, 0, 106, 111, 105, 90, 90, 118, 105, 109,
        68, 52, 76, 80, 66, 55, 100, 72, 67, 106, 116, 119, 71, 88, 86, 110, 82, 66, 118, 117, 80,
        54, 49, 67, 54, 105, 65, 87, 101, 89, 109, 78, 87, 104, 80, 0, 0, 0, 0, 104, 244, 131, 1,
        0, 0, 0, 0, 2, 0, 0, 0, 44, 0, 0, 0, 66, 114, 105, 53, 119, 122, 122, 86, 121, 66, 75, 52,
        105, 54, 121, 50, 89, 121, 69, 84, 57, 106, 87, 114, 98, 117, 67, 50, 88, 51, 69, 67, 99,
        72, 100, 66, 119, 54, 69, 49, 100, 50, 71, 110, 43, 0, 0, 0, 106, 111, 105, 90, 90, 118,
        105, 109, 68, 52, 76, 80, 66, 55, 100, 72, 67, 106, 116, 119, 71, 88, 86, 110, 82, 66, 118,
        117, 80, 54, 49, 67, 54, 105, 65, 87, 101, 89, 109, 78, 87, 104, 80, 0, 0, 0, 0, 16, 174,
        34, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 2, 2, 0, 0, 0, 44, 0, 0, 0, 66, 114, 105, 53,
        119, 122, 122, 86, 121, 66, 75, 52, 105, 54, 121, 50, 89, 121, 69, 84, 57, 106, 87, 114,
        98, 117, 67, 50, 88, 51, 69, 67, 99, 72, 100, 66, 119, 54, 69, 49, 100, 50, 71, 110, 8, 87,
        145, 0, 0, 0, 0, 0, 43, 0, 0, 0, 106, 111, 105, 90, 90, 118, 105, 109, 68, 52, 76, 80, 66,
        55, 100, 72, 67, 106, 116, 119, 71, 88, 86, 110, 82, 66, 118, 117, 80, 54, 49, 67, 54, 105,
        65, 87, 101, 89, 109, 78, 87, 104, 80, 72, 12, 246, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 9, 5, 0, 0, 0, 2, 0, 0, 0, 104, 53, 2, 0, 0, 0, 100, 52, 2, 0, 0, 0, 104, 55, 2, 0, 0,
        0, 104, 54, 2, 0, 0, 0, 104, 56, 2, 0, 0, 0, 43, 0, 0, 0, 106, 111, 105, 90, 90, 118, 105,
        109, 68, 52, 76, 80, 66, 55, 100, 72, 67, 106, 116, 119, 71, 88, 86, 110, 82, 66, 118, 117,
        80, 54, 49, 67, 54, 105, 65, 87, 101, 89, 109, 78, 87, 104, 80, 0, 168, 97, 0, 0, 0, 0, 0,
        0, 44, 0, 0, 0, 66, 114, 105, 53, 119, 122, 122, 86, 121, 66, 75, 52, 105, 54, 121, 50, 89,
        121, 69, 84, 57, 106, 87, 114, 98, 117, 67, 50, 88, 51, 69, 67, 99, 72, 100, 66, 119, 54,
        69, 49, 100, 50, 71, 110, 1, 80, 195, 0, 0, 0, 0, 0, 0, 248, 36, 1, 0, 0, 0, 0, 0, 8, 0, 0,
        0, 44, 0, 0, 0, 66, 55, 77, 121, 51, 101, 110, 72, 83, 87, 121, 87, 84, 106, 89, 105, 52,
        69, 120, 54, 107, 104, 55, 74, 69, 89, 69, 115, 69, 57, 70, 111, 80, 82, 51, 53, 115, 49,
        53, 117, 87, 67, 65, 102, 4, 240, 73, 2, 0, 0, 0, 0, 0, 44, 0, 0, 0, 69, 78, 82, 49, 49,
        118, 80, 78, 119, 50, 120, 80, 88, 107, 67, 74, 107, 49, 87, 111, 104, 101, 117, 105, 49,
        89, 114, 100, 54, 56, 78, 71, 87, 68, 66, 109, 103, 119, 83, 100, 122, 99, 119, 80, 2, 43,
        0, 0, 0, 106, 111, 105, 90, 90, 118, 105, 109, 68, 52, 76, 80, 66, 55, 100, 72, 67, 106,
        116, 119, 71, 88, 86, 110, 82, 66, 118, 117, 80, 54, 49, 67, 54, 105, 65, 87, 101, 89, 109,
        78, 87, 104, 80, 4, 24, 101, 45, 0, 0, 0, 0, 0, 44, 0, 0, 0, 66, 114, 105, 53, 119, 122,
        122, 86, 121, 66, 75, 52, 105, 54, 121, 50, 89, 121, 69, 84, 57, 106, 87, 114, 98, 117, 67,
        50, 88, 51, 69, 67, 99, 72, 100, 66, 119, 54, 69, 49, 100, 50, 71, 110, 2, 44, 0, 0, 0, 66,
        55, 77, 121, 51, 101, 110, 72, 83, 87, 121, 87, 84, 106, 89, 105, 52, 69, 120, 54, 107,
        104, 55, 74, 69, 89, 69, 115, 69, 57, 70, 111, 80, 82, 51, 53, 115, 49, 53, 117, 87, 67,
        65, 102, 4, 56, 68, 126, 0, 0, 0, 0, 0, 44, 0, 0, 0, 69, 78, 82, 49, 49, 118, 80, 78, 119,
        50, 120, 80, 88, 107, 67, 74, 107, 49, 87, 111, 104, 101, 117, 105, 49, 89, 114, 100, 54,
        56, 78, 71, 87, 68, 66, 109, 103, 119, 83, 100, 122, 99, 119, 80, 3, 43, 0, 0, 0, 106, 111,
        105, 90, 90, 118, 105, 109, 68, 52, 76, 80, 66, 55, 100, 72, 67, 106, 116, 119, 71, 88, 86,
        110, 82, 66, 118, 117, 80, 54, 49, 67, 54, 105, 65, 87, 101, 89, 109, 78, 87, 104, 80, 2,
        44, 0, 0, 0, 66, 114, 105, 53, 119, 122, 122, 86, 121, 66, 75, 52, 105, 54, 121, 50, 89,
        121, 69, 84, 57, 106, 87, 114, 98, 117, 67, 50, 88, 51, 69, 67, 99, 72, 100, 66, 119, 54,
        69, 49, 100, 50, 71, 110, 2, 104, 244, 131, 1, 0, 0, 0, 0, 2, 0, 0, 0, 43, 0, 0, 0, 106,
        111, 105, 90, 90, 118, 105, 109, 68, 52, 76, 80, 66, 55, 100, 72, 67, 106, 116, 119, 71,
        88, 86, 110, 82, 66, 118, 117, 80, 54, 49, 67, 54, 105, 65, 87, 101, 89, 109, 78, 87, 104,
        80, 1, 44, 0, 0, 0, 66, 114, 105, 53, 119, 122, 122, 86, 121, 66, 75, 52, 105, 54, 121, 50,
        89, 121, 69, 84, 57, 106, 87, 114, 98, 117, 67, 50, 88, 51, 69, 67, 99, 72, 100, 66, 119,
        54, 69, 49, 100, 50, 71, 110, 1, 104, 244, 131, 1, 0, 0, 0, 0, 2, 0, 0, 0, 43, 0, 0, 0,
        106, 111, 105, 90, 90, 118, 105, 109, 68, 52, 76, 80, 66, 55, 100, 72, 67, 106, 116, 119,
        71, 88, 86, 110, 82, 66, 118, 117, 80, 54, 49, 67, 54, 105, 65, 87, 101, 89, 109, 78, 87,
        104, 80, 1, 44, 0, 0, 0, 66, 114, 105, 53, 119, 122, 122, 86, 121, 66, 75, 52, 105, 54,
        121, 50, 89, 121, 69, 84, 57, 106, 87, 114, 98, 117, 67, 50, 88, 51, 69, 67, 99, 72, 100,
        66, 119, 54, 69, 49, 100, 50, 71, 110, 1, 104, 244, 131, 1, 0, 0, 0, 0, 2, 0, 0, 0, 43, 0,
        0, 0, 106, 111, 105, 90, 90, 118, 105, 109, 68, 52, 76, 80, 66, 55, 100, 72, 67, 106, 116,
        119, 71, 88, 86, 110, 82, 66, 118, 117, 80, 54, 49, 67, 54, 105, 65, 87, 101, 89, 109, 78,
        87, 104, 80, 0, 72, 12, 246, 0, 0, 0, 0, 0, 44, 0, 0, 0, 66, 114, 105, 53, 119, 122, 122,
        86, 121, 66, 75, 52, 105, 54, 121, 50, 89, 121, 69, 84, 57, 106, 87, 114, 98, 117, 67, 50,
        88, 51, 69, 67, 99, 72, 100, 66, 119, 54, 69, 49, 100, 50, 71, 110, 2, 0, 0, 0, 0, 0, 0, 0,
        0,
    ];

    let h = Holdem::try_from_slice(&data)?;
    println!("{:#?}", h);
    Ok(())
}
