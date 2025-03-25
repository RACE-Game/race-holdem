use super::*;

pub const DEFAULT_SB: u64 = 50;
pub const DEFAULT_BB: u64 = 100;

// Create Mtt with a number of players. If `with_tables` is
// true, also create MttTable states.
pub fn create_mtt_with_players(player_nums_per_table: &[usize], table_size: u8) -> Mtt {
    let start_chips = 10000;
    let mut mtt = Mtt {
        stage: MttStage::Playing,
        table_size,
        start_chips,
        ..Default::default()
    };

    let mut rank_id = 1;
    let mut table_id = 0;

    for player_num in player_nums_per_table {
        table_id += 1;
        let table_state = MttTableState {
            table_id,
            btn: 0,
            sb: DEFAULT_SB,
            bb: DEFAULT_BB,
            players: vec![],
            ..Default::default()
        };

        mtt.tables.insert(table_id, table_state);
        let table_state = mtt.tables.get_mut(&table_id).unwrap();

        for i in 0..*player_num {
            mtt.ranks.push(PlayerRank {
                id: rank_id,
                chips: start_chips,
                status: PlayerRankStatus::Play,
                position: rank_id as u16 % table_size as u16,
                deposit_history: vec![start_chips],
            });

            let player = MttTablePlayer::new(rank_id, start_chips, i);
            table_state.players.push(player);
            mtt.table_assigns.insert(rank_id, table_id);
            rank_id += 1;
        }
    }

    mtt
}

#[test]
fn create_mtt_with_15_players_in_3_tables() {
    let player_count = 15;
    let table_size = 6;
    let mtt = create_mtt_with_players(&[6, 6, 3], table_size);
    assert_eq!(mtt.ranks.len(), player_count);
    assert_eq!(mtt.tables.len(), 3);
    let first_table = mtt.tables.values().next().unwrap();
    let last_table = mtt.tables.values().last().unwrap();
    assert_eq!(first_table.players.len(), table_size as usize);
    assert_eq!(last_table.players.len(), 3);
}
