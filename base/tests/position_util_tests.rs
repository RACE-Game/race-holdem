use race_poker_base::game::PokerGame;
use race_poker_base::holdem::HoldemVariant;
use race_poker_base::essential::{Player, PlayerStatus};
use std::collections::BTreeMap;

#[test]
fn test_position_is_between_btn_and_sb() {
    let mut state = PokerGame::<HoldemVariant>::default();
    state.table_size = 8;
    state.btn = 7;
    state.player_map = BTreeMap::from([
        (1, Player::new_with_defaults(1, 100, 0, PlayerStatus::Init)),
        (2, Player::new_with_defaults(2, 100, 2, PlayerStatus::Init)),
        (3, Player::new_with_defaults(3, 100, 3, PlayerStatus::Init)),
    ]);
    assert_eq!(state.is_position_between_btn_and_bb(1), true);

    state.table_size = 8;
    state.btn = 0;
    state.player_map = BTreeMap::from([
        (1, Player::new_with_defaults(1, 100, 1, PlayerStatus::Init)),
        (2, Player::new_with_defaults(2, 100, 3, PlayerStatus::Init)),
        (3, Player::new_with_defaults(3, 100, 4, PlayerStatus::Init)),
    ]);
    assert_eq!(state.is_position_between_btn_and_bb(2), true);

    state.table_size = 8;
    state.btn = 0;
    state.player_map = BTreeMap::from([
        (1, Player::new_with_defaults(1, 100, 0, PlayerStatus::Init)),
        (2, Player::new_with_defaults(2, 100, 2, PlayerStatus::Init)),
        (3, Player::new_with_defaults(3, 100, 3, PlayerStatus::Init)),
    ]);
    assert_eq!(state.is_position_between_btn_and_bb(0), true);

    state.table_size = 8;
    state.btn = 0;
    state.player_map = BTreeMap::from([
        (1, Player::new_with_defaults(1, 100, 0, PlayerStatus::Init)),
        (2, Player::new_with_defaults(2, 100, 2, PlayerStatus::Init)),
        (3, Player::new_with_defaults(3, 100, 3, PlayerStatus::Init)),
    ]);
    assert_eq!(state.is_position_between_btn_and_bb(2), true);

    state.table_size = 8;
    state.btn = 0;
    state.player_map = BTreeMap::from([
        (1, Player::new_with_defaults(1, 100, 0, PlayerStatus::Init)),
        (4, Player::new_with_defaults(4, 100, 1, PlayerStatus::Waitbb)),
        (2, Player::new_with_defaults(2, 100, 3, PlayerStatus::Init)),
        (3, Player::new_with_defaults(3, 100, 4, PlayerStatus::Init)),
    ]);
    assert_eq!(state.is_position_between_btn_and_bb(2), true);

    state.table_size = 8;
    state.btn = 0;
    state.player_map = BTreeMap::from([
        (1, Player::new_with_defaults(1, 100, 0, PlayerStatus::Init)),
        (4, Player::new_with_defaults(4, 100, 2, PlayerStatus::Waitbb)),
        (2, Player::new_with_defaults(2, 100, 3, PlayerStatus::Init)),
        (3, Player::new_with_defaults(3, 100, 4, PlayerStatus::Init)),
    ]);
    assert_eq!(state.is_position_between_btn_and_bb(1), true);
}

#[test]
fn test_position_is_not_between_btn_and_sb() {
    let mut state = PokerGame::<HoldemVariant>::default();
    state.table_size = 8;
    state.btn = 7;
    state.player_map = BTreeMap::from([
        (1, Player::new_with_defaults(1, 100, 0, PlayerStatus::Init)),
        (2, Player::new_with_defaults(2, 100, 2, PlayerStatus::Init)),
        (3, Player::new_with_defaults(3, 100, 3, PlayerStatus::Init)),
    ]);
    assert_eq!(state.is_position_between_btn_and_bb(4), false);

    state.table_size = 8;
    state.btn = 7;
    state.player_map = BTreeMap::from([
        (1, Player::new_with_defaults(1, 100, 0, PlayerStatus::Init)),
        (2, Player::new_with_defaults(2, 100, 2, PlayerStatus::Init)),
        (3, Player::new_with_defaults(3, 100, 3, PlayerStatus::Init)),
    ]);
    assert_eq!(state.is_position_between_btn_and_bb(6), false);
    assert_eq!(state.is_position_between_btn_and_bb(4), false);
    assert_eq!(state.is_position_between_btn_and_bb(5), false);
    assert_eq!(state.is_position_between_btn_and_bb(0), true);
    assert_eq!(state.is_position_between_btn_and_bb(1), true);
    assert_eq!(state.is_position_between_btn_and_bb(2), true);
    assert_eq!(state.is_position_between_btn_and_bb(3), false);
}
