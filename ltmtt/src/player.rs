use borsh::{BorshDeserialize, BorshSerialize};

#[derive(Default, BorshSerialize, BorshDeserialize, Debug)]
pub enum PlayerStatus {
    #[default]
    SatIn,
    SatOut,
}

#[derive(Default, BorshSerialize, BorshDeserialize, Debug)]
pub struct Player {
    pub player_id: u64,
    pub position: u16,
    pub status: PlayerStatus,
}

#[derive(Default, BorshSerialize, BorshDeserialize, Debug)]
pub struct Ranking {
    pub player_id: u64,
    pub chips: u64,
    pub deposit_history: Vec<u64>,
}
