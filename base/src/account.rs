use borsh::{BorshSerialize, BorshDeserialize};

/// Representation of a specific on-chain Holdem game account
#[derive(BorshSerialize, BorshDeserialize, PartialEq, Debug)]
pub struct HoldemAccount {
    pub sb: u64,
    pub bb: u64,
    pub ante: u64,
    pub rake: u16,             // an integer representing the rake (per thousand)
    pub rake_cap: u8,          // the maximum rake in BB
    pub max_deposit: u64,      // the maximum deposit in chips, usually 100BB
    pub theme: Option<String>, // an optional theme identifier
}

impl Default for HoldemAccount {
    fn default() -> Self {
        Self {
            sb: 10,
            bb: 20,
            ante: 0,
            rake: 3,
            rake_cap: 1,
            max_deposit: 2000,
            theme: None,
        }
    }
}
