mod helper;

use std::collections::HashMap;

use race_holdem_base::essential::*;
use helper::{create_sync_event, setup_holdem_game};
use race_api::{error::Result, event::Event};
use race_test::prelude::*;

// Test one player reaches the maximum number of timeouts in heads up
#[test]
fn test_action_timeout() -> Result<()> {
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
        let state = handler.get_mut_state();
        state.player_map.entry("Alice".to_string()).and_modify(|p| p.timeout = MAX_ACTION_TIMEOUT_COUNT);
    }

    handler.handle_dispatch_event(&mut ctx)?;

    {
        let state = handler.get_state();
        assert_eq!(state.player_map.len(), 1);
    }
    Ok(())
}
