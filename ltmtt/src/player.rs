use borsh::{BorshDeserialize, BorshSerialize};

#[derive(Default, BorshSerialize, BorshDeserialize, Debug)]
pub struct LtMttPlayer {
    pub id: u64,
    pub chips: u64,
    // pub status: PlayerStatus,
    pub position: u16,
}
