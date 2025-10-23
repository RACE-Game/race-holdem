//! Test setup specificaly for issue #158: 3-way allin incorrect settlement
//! For more, see: https://github.com/RACE-Game/racepoker/issues/158

use race_api::prelude::*;
use race_poker_base::essential::*;
use race_poker_base::game::PokerGame;
use race_poker_base::holdem::HoldemVariant;
use race_test::prelude::*;
use std::collections::{HashMap, BTreeMap};

#[test]
fn three_way_allin_incorrect_settle() -> Result<()> {
    // snapshot of players
    let upcool = Player {
        id: 1,
        deposit: 0,
        chips: 100000,
        position: 0,
        status: PlayerStatus::Acted,
        ..Default::default()
    };
    let arco = Player {
        id: 2,
        deposit: 0,
        chips: 62300,
        position: 1,
        status: PlayerStatus::Acted,
        ..Default::default()
    };
    let ramezrko = Player {
        id: 3,
        deposit: 0,
        chips: 100000,
        position: 2,
        status: PlayerStatus::Acting,
        ..Default::default()
    };
    let sirman = Player {
        id: 4,
        deposit: 0,
        chips: 13700,
        position: 3,
        status: PlayerStatus::Wait,
        ..Default::default()
    };

    let player_map = BTreeMap::from([
        (1, upcool), // sb, raise allin
        (2, arco),   // bb, hero call
        (3, ramezrko), // utg, raise then fold
        (4, sirman), // btn, allin
    ]);

    let acting_player = Some(ActingPlayer {
        id: 3,
        position: 2,
        action_start: 0,
        clock: 25_000,
        time_card_clock: None,
    });

    let bet_map = BTreeMap::from([
        (1, 1250),
        (2, 2500),
    ]);

    let hand_index_map = BTreeMap::from([
        (1, vec![0, 1]),
        (2, vec![2, 3]),
        (3, vec![4, 5]),
        (4, vec![6, 7]),
    ]);

    let mut holdem = PokerGame::<HoldemVariant> {
        hand_id: 1,
        deck_random_id: 1,
        max_deposit: 2000,
        sb: 1250,
        bb: 2500,
        ante: 300,
        rake: 0,
        rake_cap: 0,
        stage: HoldemStage::Play,
        street: Street::Preflop,
        street_bet: 2500, // should be BB
        bet_map,
        player_map,
        player_order: vec![3, 4, 1, 2],
        table_size: 6,
        acting_player,
        mode: GameMode::Cash,
        hand_index_map,
        ..Default::default()
    };

    let utg_event = GameEvent::Raise(5000);
    let btn_event = GameEvent::Raise(13700);
    let sb_event = GameEvent::Raise(63550);
    let bb_event = GameEvent::Call;
    let utg_fold_event = GameEvent::Fold;

    // utg - 3, btn - 4, sb - 1, bb - 2

    let mut e = Effect::default();
    holdem.handle_custom_event(&mut e, utg_event, 3)?;
    let mut e = Effect::default();
    holdem.handle_custom_event(&mut e, btn_event, 4)?;
    let mut e = Effect::default();
    holdem.handle_custom_event(&mut e, sb_event, 1)?;
    let mut e = Effect::default();
    holdem.handle_custom_event(&mut e, bb_event, 2)?;
    let mut e = Effect::default();
    holdem.handle_custom_event(&mut e, utg_fold_event, 3)?;

    let mut e = Effect::default();
    let revealed = HashMap::from([
        (1, HashMap::from([
            (0,  "hq".to_string()),
            (1,  "ck".to_string()),
            (2,  "sk".to_string()),
            (3,  "sq".to_string()),
            (6,  "c8".to_string()),
            (7,  "da".to_string()),
            (8,  "s7".to_string()),
            (9,  "ct".to_string()),
            (10, "h6".to_string()),
            (11, "d2".to_string()),
            (12, "s3".to_string()),
        ]))
    ]);
    e.revealed = revealed.clone();

    assert_eq!(holdem.pots, vec![Pot {
        owners: vec![1, 2, 4],
        winners: vec![],
        amount: 46100,
    }, Pot {
        owners: vec![1, 2],
        winners: vec![],
        amount: 102200,
    }]);

    holdem.handle_event(&mut e, Event::SecretsReady { random_ids: vec![1]} )?;

    Ok(())
}
