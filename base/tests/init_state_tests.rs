use race_api::prelude::*;
use race_holdem_base::{game::Holdem, essential::HoldemAccount};

#[test]
fn test_init_state() {
    let mut init_account = InitAccount::default();
    let data = HoldemAccount::default();
    init_account.data = data.try_to_vec().unwrap();
    init_account.add_player(1, 0, 10000);
    let mut eff = Effect::default();
    let s = Holdem::init_state(&mut eff, init_account).unwrap();

    assert_eq!(s.player_map.len(), 1);
}
