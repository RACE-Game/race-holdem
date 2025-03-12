use super::*;

fn add_player_to_mtt(mtt: &mut Mtt) -> u64 {
    let l = mtt.ranks.len();
    let player_id = (l + 1) as u64;
    mtt.ranks.push(PlayerRank::new(
        player_id,
        1000,
        PlayerRankStatus::Pending,
        l as u16
    ));
    return player_id
}

#[test]
fn sit_player_given_all_tables_full_do_create_new_table() {
    let mut mtt = helper::create_mtt_with_players(&[3, 3, 3], 3);
    mtt.stage = MttStage::Playing;
    let mut effect = Effect::default();

    let pid = add_player_to_mtt(&mut mtt);
    println!("ranks: {:?}", mtt.ranks);
    mtt.sit_player(&mut effect, pid).unwrap();

    println!("Logs: {:?}", effect.logs);

    assert_eq!(effect.launch_sub_games.len(), 1);
}

#[test]
fn sit_player_given_two_table_with_one_player_do_sit_to_non_empty_table() {
    let mut mtt = helper::create_mtt_with_players(&[1, 0], 3);
    mtt.stage = MttStage::Playing;
    let mut effect = Effect::default();

    let pid = add_player_to_mtt(&mut mtt);
    mtt.sit_player(&mut effect, pid).unwrap();

    println!("Logs: {:?}", effect.logs);

    assert_eq!(effect.list_bridge_events().unwrap(), vec![
        (1,
            HoldemBridgeEvent::SitinPlayers {
                sitins: vec![MttTableSitin::new(pid, 1000)]
            }
        )
    ]);
}

#[test]
fn sit_player_given_two_table_with_one_player_do_sit_to_non_empty_table_2() {
    let mut mtt = helper::create_mtt_with_players(&[0, 1], 3);
    mtt.stage = MttStage::Playing;
    let mut effect = Effect::default();

    let pid = add_player_to_mtt(&mut mtt);
    mtt.sit_player(&mut effect, pid).unwrap();

    println!("Logs: {:?}", effect.logs);

    assert_eq!(effect.list_bridge_events().unwrap(), vec![
        (2,
            HoldemBridgeEvent::SitinPlayers {
                sitins: vec![MttTableSitin::new(pid, 1000)]
            }
        )
    ]);
}
