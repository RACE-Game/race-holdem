//! Test serializing and deserializing verious structs used by Holdem
//! as well as the Holdem struct itself

mod helper;

use std::collections::BTreeMap;

use borsh::{BorshDeserialize, BorshSerialize};
use helper::make_uneven_betmap;
use race_holdem_base::essential::{
    AwardPot, Display, GameEvent, HoldemAccount, Player, PlayerResult, PlayerStatus, Pot,
};

const ALICE: u64 = 0;
const BOB: u64 = 1;
const CAROL: u64 = 2;

#[test]
fn test_borsh_player() {
    let player = Player::new_with_timeout(ALICE, 1000, 1u16, 0);
    let player_ser = player.try_to_vec().unwrap();
    let player_de = Player::try_from_slice(&player_ser).unwrap();
    assert_eq!(player_de, player);
}

#[test]
fn test_borsh_pot() {
    let pot = Pot {
        owners: vec![ALICE, BOB, CAROL],
        winners: vec![ALICE],
        amount: 120,
    };
    let pot_ser = pot.try_to_vec().unwrap();
    let pot_de = Pot::try_from_slice(&pot_ser).unwrap();
    assert_eq!(pot_de, pot);
}

#[test]
fn test_borsh_holdem_account() {
    let acct = HoldemAccount {
        sb: 10,
        bb: 20,
        ante: 0,
        rake: 3,
        rake_cap: 1,
        theme: None,
    };
    let acct_ser = acct.try_to_vec().unwrap();
    println!("Game Account Ser data {:?}", acct_ser);
    let acct_de = HoldemAccount::try_from_slice(&acct_ser).unwrap();
    assert_eq!(acct_de, acct);
}

#[test]
fn test_borsh_game_event() {
    let evts = vec![
        GameEvent::Call,
        GameEvent::Bet(20),
        GameEvent::Fold,
        GameEvent::Check,
        GameEvent::Raise(60),
    ];
    for evt in evts.into_iter() {
        println!("Event: {:?}", evt);
        let evt_ser = evt.try_to_vec().unwrap();
        let evt_de = GameEvent::try_from_slice(&evt_ser).unwrap();
        assert_eq!(evt_de, evt);
    }
}

#[test]
fn test_borsh_display() {
    let display = vec![
        Display::DealCards,
        Display::DealBoard {
            prev: 3usize,
            board: vec![
                "sq".to_string(),
                "hq".to_string(),
                "ca".to_string(),
                "dt".to_string(),
                "c6".to_string(),
            ],
        },
        Display::GameResult {
            player_map: BTreeMap::from([
                (
                    ALICE,
                    PlayerResult {
                        id: ALICE,
                        position: 0,
                        status: PlayerStatus::Wait,
                        chips: 100,
                        prize: Some(100),
                    },
                ),
                (
                    BOB,
                    PlayerResult {
                        id: BOB,
                        position: 1,
                        status: PlayerStatus::Out,
                        chips: 0,
                        prize: None,
                    },
                ),
            ]),
        },
        Display::CollectBets {
            old_pots: vec![],
            bet_map: make_uneven_betmap(),
        },
    ];

    for dlp in display.into_iter() {
        println!("Display: {:?}", dlp);
        let dlp_ser = dlp.try_to_vec().unwrap();
        let dlp_de = Display::try_from_slice(&dlp_ser).unwrap();
        assert_eq!(dlp_de, dlp);
    }
}
