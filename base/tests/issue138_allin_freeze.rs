//! Test setup specificaly for issue #138: game freeze if allin < blind bet
//! For more, see: https://github.com/RACE-Game/racepoker/issues/138

use race_api::prelude::*;
use race_test::prelude::*;
use std::collections::BTreeMap;
use race_holdem_base::essential::*;
use race_holdem_base::game::Holdem;
use race_holdem_base::hand_history::HandHistory;

#[test]
fn allin_smaller_than_blinds() -> Result<()> {

    // snapshot of players
    let chad = Player { id: 0, deposit: 10000, chips: 13300, position: 0, status: PlayerStatus::Acted, ..Player::default() };
    let kaku = Player { id: 1, deposit: 10000, chips: 4350, position: 1, status: PlayerStatus::Acted, ..Player::default() };
    let jmp = Player { id: 2, deposit: 10000, chips: 0, position: 2, status: PlayerStatus::Acted, ..Player::default() };
    let live = Player { id: 3, deposit: 10000, chips: 9000, position: 3, status: PlayerStatus::Acting, ..Player::default() };
    let rhei = Player { id: 4, deposit: 1000, chips: 11200, position: 4, status: PlayerStatus::Wait, ..Player::default() };

    let player_map = BTreeMap::from([
        (0, chad),  // utg bet
        (1, kaku),  // mid bet
        (2, jmp),   // btn allin
        (3, live),  // sb fold
        (4, rhei),  // bb acting
    ]);

    // snapshot of acting player
    let acting_player = Some(ActingPlayer {
        id: 3, position: 3, action_start: 0, clock: 25_000, time_card_clock: None
    });

    // snapshot of pots
    let pots = vec![Pot {
        owners: vec![0, 1, 2, 3, 4],
        winners: vec![],
        amount: 2150,  // sb and bb bets not collected yet
    }];

    // snapshot of bet map
    let bet_map = BTreeMap::from([
        (0, 1000),              // utg
        (1, 1000),              // mid
        (2, 150),               // btn
        (3, 500),               // sb
        (4, 1000),              // bb
    ]);

    // snapshot of game state
    let mut game =  Holdem {
        hand_id: 1,
        deck_random_id: 1,
        max_deposit: 2000,
        sb: 500,
        bb: 1000,
        ante: 150,
        min_raise: 1000,
        btn: 2,
        rake: 10,
        rake_cap: 25,
        stage: HoldemStage::Play,
        street: Street::Preflop,
        street_bet: 150,        // incorrect street bet for testing
        board: Vec::<String>::with_capacity(5),
        hand_index_map: BTreeMap::<u64, Vec<usize>>::new(),
        bet_map,
        total_bet_map: BTreeMap::<u64, u64>::new(),
        prize_map: BTreeMap::<u64, u64>::new(),
        player_map,
        player_order: vec![3,4,0,1,2],
        pots,
        acting_player,
        winners: Vec::<u64>::new(),
        display: Vec::<Display>::new(),
        mode: GameMode::Cash,
        table_size: 6,
        hand_history: HandHistory::default(),
        next_game_start: 0,
        rake_collected: 0,
    };


    {
        let sb_event = GameEvent::Fold;
        let mut effect = Effect::default();
        game.handle_custom_event(&mut effect, sb_event, 3).unwrap();

        let bb_event = GameEvent::Call; // bb can't call and expect error
        game.handle_custom_event(&mut effect, bb_event, 4).unwrap();
        println!("Street: {:?}", game.street);
        println!("Stage: {:?}", game.stage);
    }
    Ok(())
}
