use borsh::{BorshDeserialize, BorshSerialize};

type Millis = u64;

#[derive(Default, BorshSerialize, BorshDeserialize)]
pub struct LtMttAccountData {
    pub entry_start_time: Millis,
    pub entry_close_time: Millis,
    pub settle_time: Millis,
    pub table_size: u8,
    pub ticket: u64,
    pub start_chips: u64,
    // blind_info: BlindInfo,
    // prize_rules: Vec<u8>,
    // theme: Option<String>, // optional NFT theme
    // subgame_bundle: String,
}
