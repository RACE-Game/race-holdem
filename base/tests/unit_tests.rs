//! The unit tests in this file test functions that mutate or qeury Holdem.
//! Those functions that require `Effect' as their arguments are tested in
//! event_tests.rs.  For the a complete test of Holdem games, see holdem_test.rs
//! in the same dir.

mod helper;

use helper::{
    initial_players, make_even_betmap, make_uneven_betmap, setup_context, setup_holdem_state,
};
use race_api::prelude::HandleError;
use race_holdem_base::essential::{ActingPlayer, Display};
use std::collections::BTreeMap;

#[test]
fn test_initial_btn() -> Result<(), HandleError> {
    let mut state = setup_holdem_state()?;
    state.get_next_btn()?;
    assert_eq!(state.btn, 0);
    Ok(())
}

#[test]
fn test_collect_bets_with_even_bets() -> Result<(), HandleError> {
    let mut state = setup_holdem_state()?;

    let bet_map = make_even_betmap();
    state.bet_map = bet_map;
    state.collect_bets()?;
    assert_eq!(state.pots.len(), 1);
    assert_eq!(state.pots[0].owners.len(), 5);
    assert_eq!(state.pots[0].amount, 200);
    assert_eq!(
        state.display,
        vec![Display::CollectBets {
            bet_map: make_even_betmap()
        }]
    );
    state.pots = vec![];
    Ok(())
}

#[test]
fn test_collect_bets_with_uneven_bets() -> Result<(), HandleError> {
    let mut state = setup_holdem_state()?;
    let bet_map = make_uneven_betmap();
    state.bet_map = bet_map;
    state.collect_bets()?;
    assert_eq!(state.pots.len(), 3);
    assert_eq!(state.pots[0].amount, 100); // 20 * 5
    assert_eq!(state.pots[0].owners.len(), 5);
    assert_eq!(
        state.pots[0].owners,
        vec![
            "Alice".to_string(),
            "Bob".to_string(),
            "Carol".to_string(),
            "Dave".to_string(),
            "Eva".to_string(),
        ]
    );

    assert_eq!(state.pots[1].amount, 160); // 40 * 4
    assert_eq!(state.pots[1].owners.len(), 4);
    assert_eq!(
        state.pots[1].owners,
        vec![
            "Bob".to_string(),
            "Carol".to_string(),
            "Dave".to_string(),
            "Eva".to_string(),
        ]
    );

    assert_eq!(state.pots[2].amount, 120); // 40 * 3
    assert_eq!(state.pots[2].owners.len(), 3);
    assert_eq!(
        state.pots[2].owners,
        vec!["Bob".to_string(), "Carol".to_string(), "Eva".to_string(),]
    );

    Ok(())
}

#[test]
fn test_assign_winners() -> Result<(), HandleError> {
    let mut state = setup_holdem_state()?;

    // One pot with a single winner
    {
        let bet_map = make_even_betmap();
        state.bet_map = bet_map;
        state.collect_bets()?;
        // Order of winners presents rankings of players' hands, from strong to weak
        let winners = vec![
            vec!["Bob".to_string()],
            vec!["Dave".to_string()],
            vec!["Carol".to_string()],
            vec!["Alice".to_string()],
            vec!["Eva".to_string()],
        ];
        state.assign_winners(winners)?;
        assert_eq!(state.pots.len(), 1);
        assert_eq!(state.pots[0].winners.len(), 1);
        assert_eq!(state.pots[0].winners, vec!["Bob".to_string()]);

        state.pots = vec![];
    }

    // One pot with multi-winners (draw).  This also applies to multi-pots, of which
    // each pot has a single winner
    {
        let bet_map = make_even_betmap();
        state.bet_map = bet_map;
        state.collect_bets()?;
        let winners = vec![
            vec!["Bob".to_string(), "Alice".to_string()],
            vec!["Dave".to_string()],
            vec!["Carol".to_string()],
            vec!["Eva".to_string()],
        ];
        state.assign_winners(winners)?;
        assert_eq!(state.pots.len(), 1);
        assert_eq!(state.pots[0].winners.len(), 2);
        assert_eq!(
            state.pots[0].winners,
            vec!["Bob".to_string(), "Alice".to_string()]
        );

        state.pots = vec![];
    }

    // Multi-pots and each with a single winner (also applies to multi-winners situation)
    {
        let bet_map = make_uneven_betmap();
        state.bet_map = bet_map;
        state.collect_bets()?;
        let winners = vec![
            vec!["Alice".to_string()], // winner of main pot
            vec!["Dave".to_string()],  // winner of side pot 1
            vec!["Carol".to_string()], // winner of side pot 2
            vec!["Bob".to_string()],
            vec!["Eva".to_string()],
        ];

        state.assign_winners(winners)?;

        assert_eq!(state.pots.len(), 3);
        assert_eq!(state.pots[0].winners.len(), 1);
        assert_eq!(state.pots[0].winners, vec!["Alice".to_string()]);
        assert_eq!(state.pots[1].winners.len(), 1);
        assert_eq!(state.pots[1].winners, vec!["Dave".to_string()]);
        assert_eq!(state.pots[2].winners.len(), 1);
        assert_eq!(state.pots[2].winners, vec!["Carol".to_string()]);

        state.pots = vec![];
    }

    Ok(())
}

#[test]
fn test_calc_prize() -> Result<(), HandleError> {
    let mut state = setup_holdem_state()?;
    // One pot with a single winner
    {
        let bet_map = make_even_betmap();
        state.bet_map = bet_map;
        state.collect_bets()?;
        let winners = vec![
            vec!["Bob".to_string()], // single winner
            vec!["Alice".to_string()],
            vec!["Dave".to_string()],
            vec!["Carol".to_string()],
            vec!["Eva".to_string()],
        ];
        state.assign_winners(winners)?;
        state.calc_prize()?;
        assert_eq!(state.pots.len(), 1);
        assert_eq!(state.pots[0].winners.len(), 1);
        assert_eq!(state.prize_map.len(), 2); // there's odd_chips_winner
        assert_eq!(state.prize_map.get("Bob"), Some(&200));

        state.pots = vec![];
        state.prize_map = BTreeMap::new();
    }

    // One pot with multi-winners (draw)
    {
        let bet_map = make_even_betmap();
        state.bet_map = bet_map;
        state.collect_bets()?;
        let winners = vec![
            // 3 players slipt pot and Alice gets the remainder
            vec!["Bob".to_string(), "Dave".to_string(), "Alice".to_string()],
            vec!["Carol".to_string()],
            vec!["Eva".to_string()],
        ];
        state.assign_winners(winners)?;
        state.calc_prize()?;
        assert_eq!(state.pots.len(), 1);
        assert_eq!(state.pots[0].winners.len(), 3);
        assert_eq!(state.prize_map.len(), 3);
        assert_eq!(state.prize_map.get("Bob"), Some(&66));
        assert_eq!(state.prize_map.get("Dave"), Some(&66));
        assert_eq!(state.prize_map.get("Alice"), Some(&68));

        state.pots = vec![];
        state.prize_map = BTreeMap::new();
    }

    // Multi pots and each with multip winners
    {
        let bet_map = make_uneven_betmap();
        state.bet_map = bet_map;
        state.collect_bets()?;
        let winners = vec![
            // Alice and Dave split main pot and Dave also wins side pot 1
            vec!["Dave".to_string(), "Alice".to_string()],
            // Bob wins side pot 2
            vec!["Bob".to_string()],
            vec!["Carol".to_string()],
            vec!["Eva".to_string()],
        ];
        state.assign_winners(winners)?;
        state.calc_prize()?;
        assert_eq!(state.pots.len(), 3);
        assert_eq!(state.pots[0].winners.len(), 2);
        assert_eq!(
            state.pots[0].winners,
            vec!["Dave".to_string(), "Alice".to_string()]
        );
        assert_eq!(state.pots[1].winners.len(), 1);
        assert_eq!(state.pots[1].winners, vec!["Dave".to_string()]);
        assert_eq!(state.pots[2].winners.len(), 1);
        assert_eq!(state.pots[2].winners, vec!["Bob".to_string()]);

        assert_eq!(state.prize_map.len(), 3);
        assert_eq!(state.prize_map.get("Alice"), Some(&50));
        assert_eq!(state.prize_map.get("Dave"), Some(&210));
        assert_eq!(state.prize_map.get("Bob"), Some(&120));
    }
    Ok(())
}

// NOTE: In real cases, players' chips will be decreased by the amount they bet.
// Here we skip the step of taking bets from them and focus on the prizes they get.
#[test]
fn test_apply_prize() -> Result<(), HandleError> {
    let mut state = setup_holdem_state()?;
    // One pot
    {
        let bet_map = make_even_betmap();
        state.bet_map = bet_map;
        state.collect_bets()?;
        let winners = vec![
            vec!["Bob".to_string()], // single winner
            vec!["Alice".to_string()],
            vec!["Dave".to_string()],
            vec!["Carol".to_string()],
            vec!["Eva".to_string()],
        ];
        state.assign_winners(winners)?;
        state.calc_prize()?;
        state.apply_prize()?;
        assert_eq!(state.player_map.get("Bob").unwrap().chips, 1200);

        state.pots = vec![];
        state.prize_map = BTreeMap::new();
        state.player_map = BTreeMap::from(initial_players());
    }

    // Multi-pots
    {
        let bet_map = make_uneven_betmap();
        state.bet_map = bet_map;
        state.collect_bets()?;
        let winners = vec![
            vec!["Alice".to_string()], // winner of main pot
            vec!["Dave".to_string()],  // winner of side pot 1
            vec!["Bob".to_string()],   // winner of side pot 2
            vec!["Carol".to_string()],
            vec!["Eva".to_string()],
        ];
        state.assign_winners(winners)?;
        state.calc_prize()?;
        state.apply_prize()?;
        assert_eq!(state.player_map.get("Alice").unwrap().chips, 1100);
        assert_eq!(state.player_map.get("Dave").unwrap().chips, 1160);
        assert_eq!(state.player_map.get("Bob").unwrap().chips, 1120);
    }
    Ok(())
}

// The final change of a player's chips is calculated by combining all his gains and lost
// from each pot he has betted.
#[test]
fn test_update_chips_map_singe_pot() -> Result<(), HandleError> {
    let mut state = setup_holdem_state()?;

    let bet_map = make_even_betmap();
    state.bet_map = bet_map;
    state.collect_bets()?;
    let winners = vec![
        vec!["Bob".to_string()], // single winner
        vec!["Alice".to_string()],
        vec!["Dave".to_string()],
        vec!["Carol".to_string()],
        vec!["Eva".to_string()],
    ];
    state.assign_winners(winners)?;
    state.calc_prize()?;
    let chips_change_map = state.update_chips_map()?;
    assert_eq!(chips_change_map.get("Bob"), Some(&200));
    assert_eq!(chips_change_map.get("Alice"), Some(&0));
    assert_eq!(chips_change_map.get("Dave"), Some(&0));
    assert_eq!(chips_change_map.get("Carol"), Some(&0));
    assert_eq!(chips_change_map.get("Eva"), Some(&0));
    let Some(Display::GameResult{ player_map }) = state.display.iter().find(|d| matches!(d, Display::GameResult { .. }))
        else {
            panic!("GameResult display is missing");
        };
    assert_eq!(player_map.get("Bob").unwrap().prize, Some(200));
    assert_eq!(player_map.get("Alice").unwrap().prize, None);
    assert_eq!(player_map.get("Dave").unwrap().prize, None);
    assert_eq!(player_map.get("Carol").unwrap().prize, None);
    assert_eq!(player_map.get("Eva").unwrap().prize, None);
    state.pots = vec![];
    state.prize_map = BTreeMap::new();

    Ok(())
}

#[test]
fn test_update_chips_map_with_multiple_pot() -> Result<(), HandleError> {
    let mut state = setup_holdem_state()?;

    let bet_map = make_uneven_betmap();
    state.bet_map = bet_map;
    // [20 * 1, 60 * 1, 100 * 3] ==> [[20 * 5], [40 * 4], [40 * 3]]
    state.collect_bets()?;
    let winners = vec![
        vec!["Alice".to_string()], // winner of main pot
        vec!["Dave".to_string()],  // winner of side pot 1
        vec!["Bob".to_string()],   // winner of side pot 2
        vec!["Carol".to_string()],
        vec!["Eva".to_string()],
    ];
    state.assign_winners(winners)?;
    state.calc_prize()?;
    let chips_change_map = state.update_chips_map()?;

    assert_eq!(chips_change_map.get("Alice"), Some(&100));
    assert_eq!(chips_change_map.get("Dave"), Some(&160));
    assert_eq!(chips_change_map.get("Bob"), Some(&120));
    assert_eq!(chips_change_map.get("Carol"), Some(&0));
    assert_eq!(chips_change_map.get("Eva"), Some(&0));

    for (addr, chips_change) in chips_change_map.iter() {
        if *chips_change > 0 {
            println!("Player + chips {:?}", *chips_change as u64);
            assert!(matches!(addr.as_str(), "Alice" | "Dave" | "Bob"));
        } else if *chips_change < 0 {
            println!("Player - chips {:?}", -*chips_change as u64);
            assert!(matches!(addr.as_str(), "Carol" | "Eva"));
        }
    }
    // println!("-- Display {:?}", state.display);
    let Some(Display::GameResult { player_map }) = state.display.iter().find(|d| matches!(d, Display::GameResult {..})) else {
        panic!("GameResult display not found");
    };
    assert_eq!(player_map.get("Alice").unwrap().prize, Some(100));
    assert_eq!(player_map.get("Dave").unwrap().prize, Some(160));
    assert_eq!(player_map.get("Bob").unwrap().prize, Some(120));
    assert_eq!(player_map.get("Carol").unwrap().prize, None);
    assert_eq!(player_map.get("Eva").unwrap().prize, None);
    Ok(())
}

#[test]
fn test_blind_bets() -> Result<(), HandleError> {
    let mut state = setup_holdem_state()?;
    let ctx = setup_context();
    // Effect is required to dispatch action timeout event
    let mut efx = ctx.derive_effect();

    state.blind_bets(&mut efx)?;
    assert_eq!(
        state.acting_player,
        Some(ActingPlayer {
            addr: "Dave".to_string(),
            position: 3usize,
            clock: 0,
        })
    );
    assert_eq!(state.bet_map.len(), 2);
    assert_eq!(state.bet_map.get("Bob"), Some(&state.sb));
    assert_eq!(state.bet_map.get("Carol"), Some(&state.bb));
    Ok(())
}
