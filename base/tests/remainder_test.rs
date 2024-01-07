mod helper;

use std::collections::HashMap;

use helper::{create_sync_event, setup_holdem_game};
use race_api::prelude::Event;
use race_holdem_base::essential::*;
use race_test::prelude::*;

#[test]
fn test_remainder() -> Result<()> {
    let (_, mut game_acct, mut ctx, mut handler, mut transactor) = setup_holdem_game();

    let mut alice = TestClient::player("Alice");
    let mut bob = TestClient::player("Bob"); // BTN
    let mut carol = TestClient::player("Carol");

    let mut sync_evt = create_sync_event(&mut ctx, &mut game_acct, vec![&mut alice, &mut bob, &mut carol], &transactor);

    {
        match &mut sync_evt {
            Event::Join { players, .. } => {
                players[0].balance = 777;
                players[1].balance = 777;
                players[2].balance = 777;
            }
            _ => (),
        }
    }

    handler.handle_until_no_events(
        &mut ctx,
        &sync_evt,
        vec![&mut alice, &mut bob, &mut carol, &mut transactor],
    )?;

    {
        let runner_revealed = HashMap::from([
            // Alice
            (0, "st".to_string()),
            (1, "ct".to_string()),
            // Bob
            (2, "ht".to_string()),
            (3, "dt".to_string()),
            // Carol
            (4, "h7".to_string()),
            (5, "d2".to_string()),
            // Board
            (6, "s5".to_string()),
            (7, "c6".to_string()),
            (8, "h2".to_string()),
            (9, "h8".to_string()),
            (10, "d7".to_string()),
        ]);
        let holdem_state = handler.get_state();
        ctx.add_revealed_random(holdem_state.deck_random_id, runner_revealed)?;
    }

    let evts = [
        bob.custom_event(GameEvent::Raise(111)),
        carol.custom_event(GameEvent::Call),
        alice.custom_event(GameEvent::Raise(777)),
        bob.custom_event(GameEvent::Call),
        carol.custom_event(GameEvent::Fold),
    ];

    for evt in evts {
        handler.handle_until_no_events(
            &mut ctx,
            &evt,
            vec![&mut alice, &mut bob, &mut carol, &mut transactor],
        )?;
    }

    {
        let state = handler.get_state();
        assert_eq!(state.prize_map.get(&alice.id()), Some(&831u64));
        assert_eq!(state.prize_map.get(&bob.id()), Some(&830u64));
        assert_eq!(state.stage, HoldemStage::Runner);
        assert_eq!(state.street, Street::Showdown);
    }

    Ok(())
}
