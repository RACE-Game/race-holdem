//! Test new players joining a cash table to get `Waitbb` status and to start
//! playing only when they can become the big blind for the next hand.

mod helper;
use helper::setup_holdem_game;
use race_holdem_base::game::Holdem;
use race_api::prelude::*;
use race_holdem_base::essential::*;
use race_test::prelude::*;
use race_holdem_base::hand_history::HandHistory;
use std::collections::BTreeMap;

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

// SB < NP < BB, so NP should become the actual BB
#[test]
fn test_waitbb_inbetween() -> HandleResult<()> {
    //  players
    let alice = Player { id: 1, position: 1, status: PlayerStatus::Wait, ..Player::default() }; // sb
    let bob = Player { id: 6, position: 6, status: PlayerStatus::Wait, ..Player::default() }; // calculated bb
    let carol = Player { id: 2,position: 0, status: PlayerStatus::Wait, ..Player::default() }; // next btn
    let dave = Player { id: 7, position: 3, status: PlayerStatus::Waitbb, .. Player::default() };   // actual bb

    let player_map = BTreeMap::from([
        (1, alice),
        (6, bob),
        (2, carol),
        (7, dave)
    ]);

    // snapshot of game state
    let mut game =  Holdem {
        hand_id: 1,
        deck_random_id: 1,
        max_deposit: 2000,
        sb: 100,
        bb: 200,
        ante: 20,
        min_raise: 1000,
        btn: 7,
        rake: 10,
        rake_cap: 25,
        stage: HoldemStage::Play,
        street: Street::Preflop,
        street_bet: 200,
        board: Vec::<String>::with_capacity(5),
        hand_index_map: BTreeMap::<u64, Vec<usize>>::new(),
        bet_map: BTreeMap::<u64, u64>::new(),
        total_bet_map: BTreeMap::<u64, u64>::new(),
        prize_map: BTreeMap::<u64, u64>::new(),
        player_map,
        player_order: vec![3,4,0,1,2],
        pots: Vec::<Pot>::new(),
        acting_player: None,
        winners: Vec::<u64>::new(),
        display: Vec::<Display>::new(),
        mode: GameMode::Cash,
        table_size: 9,
        hand_history: HandHistory::default(),
        next_game_start: 0,
        rake_collected: 0,
    };

    {
        let event = Event::GameStart;
        let mut effect = Effect::default();
        game.handle_event(&mut effect, event).unwrap();

        let dave = game.player_map.get(&7).unwrap();
        assert_eq!(dave.status, PlayerStatus::Wait);
        assert_eq!(game.player_order, vec![1, 7, 6, 2]);
    }

    Ok(())
}

// NP > SB > BB: new player sits at the position after (to the left of) SB,
// thus NP should become the actual BB
#[test]
fn test_waitbb_after_sb() -> HandleResult<()> {
    //  players
    let alice = Player { id: 6, position: 6, status: PlayerStatus::Wait, ..Player::default() }; // sb
    let bob = Player { id: 1, position: 1, status: PlayerStatus::Wait, ..Player::default() }; // calculated bb
    let carol = Player { id: 5,position: 5, status: PlayerStatus::Wait, ..Player::default() }; // next btn
    let dave = Player { id: 7, position: 7, status: PlayerStatus::Waitbb, .. Player::default() };   // actual bb

    let player_map = BTreeMap::from([
        (6, alice),
        (1, bob),
        (5, carol),
        (7, dave)
    ]);

    // snapshot of game state
    let mut game =  Holdem {
        hand_id: 1,
        deck_random_id: 1,
        max_deposit: 2000,
        sb: 100,
        bb: 200,
        ante: 20,
        min_raise: 1000,
        btn: 4,
        rake: 10,
        rake_cap: 25,
        stage: HoldemStage::Play,
        street: Street::Preflop,
        street_bet: 200,
        board: Vec::<String>::with_capacity(5),
        hand_index_map: BTreeMap::<u64, Vec<usize>>::new(),
        bet_map: BTreeMap::<u64, u64>::new(),
        total_bet_map: BTreeMap::<u64, u64>::new(),
        prize_map: BTreeMap::<u64, u64>::new(),
        player_map,
        player_order: vec![],
        pots: Vec::<Pot>::new(),
        acting_player: None,
        winners: Vec::<u64>::new(),
        display: Vec::<Display>::new(),
        mode: GameMode::Cash,
        table_size: 9,
        hand_history: HandHistory::default(),
        next_game_start: 0,
        rake_collected: 0,
    };

    {
        let event = Event::GameStart;
        let mut effect = Effect::default();
        game.handle_event(&mut effect, event).unwrap();

        let dave = game.player_map.get(&7).unwrap();
        assert_eq!(dave.status, PlayerStatus::Wait);
        assert_eq!(game.player_order, vec![6, 7, 1, 5]);
    }

    Ok(())
}

// SB > BB > NP: new player sits at the position before (to the right of) BB
// thus NP should become the actual BB
#[test]
fn test_waitbb_before_bb() -> HandleResult<()> {
    // players
    let alice = Player { id: 6, position: 6, status: PlayerStatus::Wait, ..Player::default() }; // sb
    let bob = Player { id: 3, position: 3, status: PlayerStatus::Wait, ..Player::default() }; // calculated bb
    let carol = Player { id: 5,position: 5, status: PlayerStatus::Wait, ..Player::default() }; // next btn
    let dave = Player { id: 7, position: 1, status: PlayerStatus::Waitbb, .. Player::default() };   // actual bb

    let player_map = BTreeMap::from([
        (6, alice),
        (3, bob),
        (5, carol),
        (7, dave)
    ]);

    // snapshot of game state
    let mut game =  Holdem {
        hand_id: 1,
        deck_random_id: 1,
        max_deposit: 2000,
        sb: 100,
        bb: 200,
        ante: 20,
        min_raise: 1000,
        btn: 4,
        rake: 10,
        rake_cap: 25,
        stage: HoldemStage::Play,
        street: Street::Preflop,
        street_bet: 200,
        board: Vec::<String>::with_capacity(5),
        hand_index_map: BTreeMap::<u64, Vec<usize>>::new(),
        bet_map: BTreeMap::<u64, u64>::new(),
        total_bet_map: BTreeMap::<u64, u64>::new(),
        prize_map: BTreeMap::<u64, u64>::new(),
        player_map,
        player_order: vec![],
        pots: Vec::<Pot>::new(),
        acting_player: None,
        winners: Vec::<u64>::new(),
        display: Vec::<Display>::new(),
        mode: GameMode::Cash,
        table_size: 9,
        hand_history: HandHistory::default(),
        next_game_start: 0,
        rake_collected: 0,
    };

    {
        let event = Event::GameStart;
        let mut effect = Effect::default();
        game.handle_event(&mut effect, event).unwrap();

        let dave = game.player_map.get(&7).unwrap();
        assert_eq!(dave.status, PlayerStatus::Wait);
        assert_eq!(game.player_order, vec![6, 7, 3, 5]);
    }

    Ok(())
}

// Test the game scenario where there are only two eligible players in the game
// 1. one player with `Wait` status
// 2. one player with `Waitbb` status
// `Wait` player becomes sb and btn and `Waitbb` player becomes bb with `Wait`
#[test]
fn test_wait_waitbb_headsup() -> HandleResult<()> {
    // players
    let alice = Player { id: 3, position: 6, status: PlayerStatus::Wait, ..Player::default() }; // sb and btn
    let bob = Player { id: 6, position: 3, status: PlayerStatus::Waitbb, ..Player::default() }; // new bb

    let player_map = BTreeMap::from([
        (3, alice),
        (6, bob),
    ]);

    // snapshot of game state
    let mut game =  Holdem {
        hand_id: 1,
        deck_random_id: 1,
        max_deposit: 2000,
        sb: 100,
        bb: 200,
        ante: 20,
        min_raise: 1000,
        btn: 4,
        rake: 10,
        rake_cap: 25,
        stage: HoldemStage::Play,
        street: Street::Preflop,
        street_bet: 200,
        board: Vec::<String>::with_capacity(5),
        hand_index_map: BTreeMap::<u64, Vec<usize>>::new(),
        bet_map: BTreeMap::<u64, u64>::new(),
        total_bet_map: BTreeMap::<u64, u64>::new(),
        prize_map: BTreeMap::<u64, u64>::new(),
        player_map,
        player_order: vec![],
        pots: Vec::<Pot>::new(),
        acting_player: None,
        winners: Vec::<u64>::new(),
        display: Vec::<Display>::new(),
        mode: GameMode::Cash,
        table_size: 6,
        hand_history: HandHistory::default(),
        next_game_start: 0,
        rake_collected: 0,
    };

    {
        let event = Event::GameStart;
        let mut effect = Effect::default();
        game.handle_event(&mut effect, event).unwrap();

        let alice = game.player_map.get(&3).unwrap();
        let bob = game.player_map.get(&6).unwrap();
        assert_eq!(alice.status, PlayerStatus::Wait);
        assert_eq!(bob.status, PlayerStatus::Wait);
        assert_eq!(game.btn, 6);
        assert_eq!(game.player_order, vec![3, 6]);
    }

    Ok(())
}


// Test the game scenario where there are following eligbile players:
// 1. one player with `Wait` status
// 2. two or more players with `Waitbb` status
// The only `Wait` player should become the next btn
// All `Waitbb` players should be added to the game and
// sb and bb are selected from them
#[test]
fn test_one_wait_multi_waitbbs() -> HandleResult<()> {
    // players
    let alice = Player { id: 3, position: 6, status: PlayerStatus::Wait, ..Player::default() }; // sb and btn
    let bob = Player { id: 6, position: 3, status: PlayerStatus::Waitbb, ..Player::default() }; // new bb
    let carol = Player { id: 8,position: 0, status: PlayerStatus::Waitbb, ..Player::default() }; // next btn
    let dave = Player { id: 9, position: 4, status: PlayerStatus::Waitbb, .. Player::default() };   // actual bb

    let player_map = BTreeMap::from([
        (3, alice),
        (6, bob),
        (8, carol),
        (9, dave),
    ]);

    // snapshot of game state
    let mut game =  Holdem {
        hand_id: 1,
        deck_random_id: 1,
        max_deposit: 2000,
        sb: 100,
        bb: 200,
        ante: 20,
        min_raise: 1000,
        btn: 4,
        rake: 10,
        rake_cap: 25,
        stage: HoldemStage::Play,
        street: Street::Preflop,
        street_bet: 200,
        board: Vec::<String>::with_capacity(5),
        hand_index_map: BTreeMap::<u64, Vec<usize>>::new(),
        bet_map: BTreeMap::<u64, u64>::new(),
        total_bet_map: BTreeMap::<u64, u64>::new(),
        prize_map: BTreeMap::<u64, u64>::new(),
        player_map,
        player_order: vec![],
        pots: Vec::<Pot>::new(),
        acting_player: None,
        winners: Vec::<u64>::new(),
        display: Vec::<Display>::new(),
        mode: GameMode::Cash,
        table_size: 6,
        hand_history: HandHistory::default(),
        next_game_start: 0,
        rake_collected: 0,
    };

    {
        let event = Event::GameStart;
        let mut effect = Effect::default();
        game.handle_event(&mut effect, event).unwrap();

        let _alice = game.player_map.get(&3).unwrap();
        let _bob = game.player_map.get(&6).unwrap();
        let _carol = game.player_map.get(&8).unwrap();
        let _dave = game.player_map.get(&9).unwrap();
        assert!(game.player_map.values().all(|p| matches!(p.status, PlayerStatus::Wait)));
        assert_eq!(game.btn, 6);
        // TODO: test who are sb and bb, respectively
    }

    Ok(())
}

// Test the game scenario where all eligbile players are with `Waitbb` status
// They should all be added to the game with `Wait` status
#[test]
fn test_multi_waitbbs_without_wait() -> HandleResult<()> {
    // players
    let alice = Player { id: 3, position: 6, status: PlayerStatus::Waitbb, ..Player::default() };
    let bob = Player { id: 6, position: 3, status: PlayerStatus::Waitbb, ..Player::default() };
    let carol = Player { id: 8,position: 0, status: PlayerStatus::Waitbb, ..Player::default() };
    let dave = Player { id: 9, position: 4, status: PlayerStatus::Waitbb, .. Player::default() };

    let player_map = BTreeMap::from([
        (3, alice),
        (6, bob),
        (8, carol),
        (9, dave),
    ]);

    // snapshot of game state
    let mut game =  Holdem {
        hand_id: 1,
        deck_random_id: 1,
        max_deposit: 2000,
        sb: 100,
        bb: 200,
        ante: 20,
        min_raise: 1000,
        btn: 4,
        rake: 10,
        rake_cap: 25,
        stage: HoldemStage::Play,
        street: Street::Preflop,
        street_bet: 200,
        board: Vec::<String>::with_capacity(5),
        hand_index_map: BTreeMap::<u64, Vec<usize>>::new(),
        bet_map: BTreeMap::<u64, u64>::new(),
        total_bet_map: BTreeMap::<u64, u64>::new(),
        prize_map: BTreeMap::<u64, u64>::new(),
        player_map,
        player_order: vec![],
        pots: Vec::<Pot>::new(),
        acting_player: None,
        winners: Vec::<u64>::new(),
        display: Vec::<Display>::new(),
        mode: GameMode::Cash,
        table_size: 6,
        hand_history: HandHistory::default(),
        next_game_start: 0,
        rake_collected: 0,
    };

    {
        let event = Event::GameStart;
        let mut effect = Effect::default();
        game.handle_event(&mut effect, event).unwrap();

        let _alice = game.player_map.get(&3).unwrap();
        let _bob = game.player_map.get(&6).unwrap();
        let _carol = game.player_map.get(&8).unwrap();
        let _dave = game.player_map.get(&9).unwrap();
        assert!(game.player_map.values().all(|p| matches!(p.status, PlayerStatus::Wait)));
        assert_eq!(game.btn, 4);
        // TODO: test who are sb and bb, respectively?
    }

    Ok(())
}

// No `Waitbb` players so `Wait` players move on as usual
#[test]
fn test_multi_wait_without_waitbb() -> HandleResult<()> {
    // players
    let alice = Player { id: 3, position: 6, status: PlayerStatus::Wait, ..Player::default() };
    let bob = Player { id: 6, position: 3, status: PlayerStatus::Wait, ..Player::default() };
    let carol = Player { id: 8,position: 0, status: PlayerStatus::Wait, ..Player::default() };
    let dave = Player { id: 9, position: 4, status: PlayerStatus::Wait, .. Player::default() };

    let player_map = BTreeMap::from([
        (3, alice),
        (6, bob),
        (8, carol),
        (9, dave),
    ]);

    // snapshot of game state
    let mut game =  Holdem {
        hand_id: 1,
        deck_random_id: 1,
        max_deposit: 2000,
        sb: 100,
        bb: 200,
        ante: 20,
        min_raise: 1000,
        btn: 4,
        rake: 10,
        rake_cap: 25,
        stage: HoldemStage::Play,
        street: Street::Preflop,
        street_bet: 200,
        board: Vec::<String>::with_capacity(5),
        hand_index_map: BTreeMap::<u64, Vec<usize>>::new(),
        bet_map: BTreeMap::<u64, u64>::new(),
        total_bet_map: BTreeMap::<u64, u64>::new(),
        prize_map: BTreeMap::<u64, u64>::new(),
        player_map,
        player_order: vec![],
        pots: Vec::<Pot>::new(),
        acting_player: None,
        winners: Vec::<u64>::new(),
        display: Vec::<Display>::new(),
        mode: GameMode::Cash,
        table_size: 6,
        hand_history: HandHistory::default(),
        next_game_start: 0,
        rake_collected: 0,
    };

    {
        let event = Event::GameStart;
        let mut effect = Effect::default();
        game.handle_event(&mut effect, event).unwrap();

        assert!(game.player_map.values().all(|p| matches!(p.status, PlayerStatus::Wait)));
        assert_eq!(game.btn, 6);
    }

    Ok(())
}
