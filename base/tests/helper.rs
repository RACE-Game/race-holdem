#![allow(dead_code)]

//! Helper functions used in tests

use std::collections::BTreeMap;

use borsh::BorshSerialize;
use race_api::prelude::*;
use race_holdem_base::hand_history::HandHistory;
use race_test::prelude::*;

use race_holdem_base::essential::*;
use race_holdem_base::game::*;

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
        (ALICE, Player::new(ALICE, 1000, 0u16, 0)),
        (BOB, Player::new(BOB, 1000, 1u16, 0)),
    ])
}

pub fn initial_players() -> BTreeMap<u64, Player> {
    BTreeMap::from([
        (ALICE, Player::new(ALICE, 1000, 0u16, 0)),
        (BOB, Player::new(BOB, 1000, 1u16, 0)),
        (CAROL, Player::new(CAROL, 1000, 2u16, 0)),
        (DAVE, Player::new(DAVE, 1000, 3u16, 0)),
        (EVA, Player::new(EVA, 1000, 4u16, 0)),
        (FRANK, Player::new(FRANK, 1000, 5u16, 0)),
    ])
}

pub fn gaming_players() -> BTreeMap<u64, Player> {
    BTreeMap::from([
        (
            ALICE,
            Player::new_with_status(ALICE, 1000, 0usize, PlayerStatus::Acting),
        ),
        (
            BOB,
            Player::new_with_status(BOB, 200, 1usize, PlayerStatus::Acted),
        ),
        (
            CAROL,
            Player::new_with_status(CAROL, 0, 2usize, PlayerStatus::Allin),
        ),
        (
            DAVE,
            Player::new_with_status(DAVE, 780, 3usize, PlayerStatus::Acted),
        ),
        (
            EVA,
            Player::new_with_status(EVA, 650, 4usize, PlayerStatus::Acted),
        ),
        (
            FRANK,
            Player::new_with_status(FRANK, 800, 5usize, PlayerStatus::Fold),
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

// Set up a holdem state with multi players joined
pub fn setup_holdem_state() -> Result<Holdem> {
    let players_map = initial_players();
    let mut state = Holdem {
        deck_random_id: 1,
        sb: 10,
        bb: 20,
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
    };
    state.arrange_players(0usize)?;
    Ok(state)
}

// Set up a holdem state with two players joined
pub fn setup_two_player_holdem() -> Result<Holdem> {
    let players_map = initial_two_players();
    let mut state = Holdem {
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
        ..Default::default()
    };
    state.arrange_players(0usize)?;
    Ok(state)
}

// Set up a holdem scene similar to those in real world
pub fn setup_real_holdem() -> Holdem {
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
        position: 1usize,
        clock: 30_000u64,
    });
    holdem
}

pub fn setup_context() -> GameContext {
    let mut transactor = TestClient::transactor("foo");
    let game_account = TestGameAccountBuilder::default()
        .set_transactor(&mut transactor)
        .build();
    let context = GameContext::try_new(&game_account).unwrap();
    context
}

// ====================================================
// Helpers for testing Holdem with the race protocol
// ====================================================
type Game = (
    InitAccount,
    GameAccount,
    GameContext,
    TestHandler<Holdem>,
    TestClient,
);

pub fn setup_holdem_game() -> Game {
    let holdem_account = HoldemAccount::default();
    let holdem_data = holdem_account.try_to_vec().unwrap();
    let mut transactor = TestClient::transactor("foo");
    let mut game_account = TestGameAccountBuilder::default()
        .with_max_players(9)
        .set_transactor(&mut transactor)
        .build();
    game_account.data = holdem_data;

    let init_account = game_account.derive_init_account();
    let mut context = GameContext::try_new(&game_account).unwrap();
    let handler = TestHandler::<Holdem>::init_state(&mut context, &game_account).unwrap();
    (init_account, game_account, context, handler, transactor)
}

pub fn create_sync_event(
    mut ctx: &mut GameContext,
    mut game_account: &mut GameAccount,
    new_players: Vec<&mut TestClient>,
    transactor: &TestClient,
) -> Event {
    ctx.add_node(
        transactor.addr(),
        ctx.get_access_version(),
        ClientMode::Transactor,
    );
    let mut players = Vec::new();
    new_players
        .into_iter()
        .for_each(|p| players.push(p.join(&mut ctx, &mut game_account, 10_000).unwrap()));

    Event::Join { players }
}
