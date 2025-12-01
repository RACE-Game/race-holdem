#![allow(dead_code)]

//! Helper functions used in tests

use std::collections::BTreeMap;

use race_api::prelude::*;
use race_poker_base::hand_history::HandHistory;
use race_test::prelude::*;
use race_poker_base::account::HoldemAccount;

use race_poker_base::essential::*;
use race_poker_base::game::*;
use race_poker_base::holdem::HoldemVariant;

const ALICE: u64 = 0;
const BOB: u64 = 1;
const CAROL: u64 = 2;
const DAVE: u64 = 3;
const EVA: u64 = 4;
const FRANK: u64 = 5;

// ======================================================
// Heplers for unit tests that focus on holdem game state
// ======================================================
pub fn initial_two_players() -> BTreeMap<u64, Player> {
    BTreeMap::from([
        (ALICE, Player::new_with_timeout(ALICE, 1000, 0, 0)),
        (BOB, Player::new_with_timeout(BOB, 1000, 1, 0)),
    ])
}

pub fn initial_players() -> BTreeMap<u64, Player> {
    BTreeMap::from([
        (ALICE, Player::new_with_timeout(ALICE, 1000, 0, 0)),
        (BOB, Player::new_with_timeout(BOB, 1000, 1, 0)),
        (CAROL, Player::new_with_timeout(CAROL, 1000, 2, 0)),
        (DAVE, Player::new_with_timeout(DAVE, 1000, 3, 0)),
        (EVA, Player::new_with_timeout(EVA, 1000, 4, 0)),
        (FRANK, Player::new_with_timeout(FRANK, 1000, 5, 0)),
    ])
}

pub fn gaming_players() -> BTreeMap<u64, Player> {
    BTreeMap::from([
        (
            ALICE,
            Player::new_with_defaults(ALICE, 1000, 0, PlayerStatus::Acting),
        ),
        (
            BOB,
            Player::new_with_defaults(BOB, 200, 1, PlayerStatus::Acted),
        ),
        (
            CAROL,
            Player::new_with_defaults(CAROL, 0, 2, PlayerStatus::Allin),
        ),
        (
            DAVE,
            Player::new_with_defaults(DAVE, 780, 3, PlayerStatus::Acted),
        ),
        (
            EVA,
            Player::new_with_defaults(EVA, 650, 4, PlayerStatus::Acted),
        ),
        (
            FRANK,
            Player::new_with_defaults(FRANK, 800, 5, PlayerStatus::Fold),
        ),
    ])
}

pub fn make_even_betmap() -> BTreeMap<u64, u64> {
    BTreeMap::from([
        (ALICE, 40u64),
        (BOB, 40u64),
        (CAROL, 40u64),
        (DAVE, 40u64),
        (EVA, 40u64),
    ])
}

pub fn make_uneven_betmap() -> BTreeMap<u64, u64> {
    BTreeMap::from([
        (ALICE, 20u64),
        (BOB, 100u64),
        (CAROL, 100u64),
        (DAVE, 60u64),
        (EVA, 100u64),
    ])
}

pub fn make_prize_map() -> BTreeMap<u64, u64> {
    BTreeMap::from([(BOB, 220u64), (CAROL, 160u64)])
}

pub fn make_pots() -> Vec<Pot> {
    vec![
        Pot {
            owners: vec![ALICE, BOB, CAROL, DAVE, EVA],
            winners: vec![],
            amount: 100u64,
        },
        Pot {
            owners: vec![BOB, CAROL, DAVE, EVA],
            winners: vec![],
            amount: 120u64,
        },
    ]
}

// Set up a initial holdem state with multi players joined
pub fn setup_holdem_state() -> Result<PokerGame<HoldemVariant>> {
    let players_map = initial_players();
    let mut state = PokerGame::<HoldemVariant> {
        hand_id: 1,
        deck_random_id: 1,
        max_deposit: 600,
        sb: 10,
        bb: 20,
        ante: 0,
        min_raise: 20,
        btn: 0,
        rake: 3,
        rake_cap: 1,
        stage: HoldemStage::Init,
        street: Street::Init,
        street_bet: 20,
        board: Vec::<String>::with_capacity(5),
        hand_index_map: BTreeMap::<u64, Vec<usize>>::new(),
        bet_map: BTreeMap::<u64, u64>::new(),
        total_bet_map: BTreeMap::<u64, u64>::new(),
        prize_map: BTreeMap::<u64, u64>::new(),
        player_map: players_map,
        player_order: Vec::<u64>::new(),
        pots: Vec::<Pot>::new(),
        acting_player: None,
        winners: Vec::<u64>::new(),
        display: Vec::<Display>::new(),
        mode: GameMode::Cash,
        table_size: 7,
        hand_history: HandHistory::default(),
        next_game_start: 0,
        rake_collected: 6000,
        variant: HoldemVariant {},
        max_afk_hands: 100,
    };
    state.arrange_players(0)?;
    Ok(state)
}

// Set up a holdem state for headsup
pub fn setup_two_player_holdem() -> Result<PokerGame<HoldemVariant>> {
    let players_map = initial_two_players();
    let mut state = PokerGame::<HoldemVariant> {
        hand_id: 1,
        deck_random_id: 1,
        sb: 10,
        bb: 20,
        min_raise: 20,
        btn: 0,
        rake: 3,
        stage: HoldemStage::Init,
        street: Street::Init,
        street_bet: 20,
        player_map: players_map,
        table_size: 6,
        mode: GameMode::Cash,
        max_deposit: 6000,
        ..Default::default()
    };
    state.arrange_players(0)?;
    Ok(state)
}

// Set up a holdem scene similar to those in real world
pub fn setup_real_holdem() -> PokerGame::<HoldemVariant> {
    let mut holdem = setup_holdem_state().unwrap();
    let player_map = gaming_players();
    let bet_map = make_even_betmap();
    let pots = make_pots();
    let board = vec![
        "sa".into(),
        "dt".into(),
        "c9".into(),
        "c2".into(),
        "hq".into(),
    ];
    let prize_map = make_prize_map();
    holdem.bet_map = bet_map;
    holdem.board = board;
    holdem.player_map = player_map;
    holdem.prize_map = prize_map;
    holdem.pots = pots;
    holdem.acting_player = Some(ActingPlayer {
        id: BOB,
        position: 1,
        clock: 30_000u64,
        action_start: 0,
        time_card_clock: None,
    });
    holdem
}


// ====================================================
// Helpers for testing Holdem with the race protocol
// ====================================================
type Game = (
    InitAccount,
    GameAccount,
    GameContext,
    TestHandler<PokerGame::<HoldemVariant>>,
    TestClient,
);

pub fn setup_holdem_game(transactor: &mut TestClient) -> TestContext<PokerGame::<HoldemVariant>> {
    let holdem_account = HoldemAccount::default();
    let (test_context, _) = TestContextBuilder::default()
        .with_max_players(9)
        .set_transactor(transactor)
        .with_deposit_range(1, 1000000000)
        .with_data(&holdem_account)
        .build_with_init_state().unwrap();

    test_context
}
