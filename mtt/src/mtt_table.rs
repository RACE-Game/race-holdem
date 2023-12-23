//! Table (wrapper of Holdem) for use by Mtt
use borsh::{BorshDeserialize, BorshSerialize};
use race_api::prelude::*;
use race_holdem_base::game::Holdem;
use race_proc_macro::game_handler;

#[game_handler]
#[derive(BorshSerialize, BorshDeserialize, Default)]
pub struct MttTable {
    pub table_id: u8,
    pub holdem: Holdem,
}

#[derive(BorshSerialize, BorshDeserialize, Default)]
pub struct MttTableCheckpoint {}

impl GameHandler for MttTable {
    type Checkpoint = MttTableCheckpoint;

    fn init_state(effect: &mut Effect, init_account: InitAccount) -> HandleResult<Self> {
        Ok(Self {table_id: 0, holdem: Holdem::init_state(effect, init_account)?})
    }

    fn handle_event(&mut self, effect: &mut Effect, event: Event) -> HandleResult<()> {
        todo!()
    }

    fn into_checkpoint(self) -> HandleResult<Self::Checkpoint> {
        todo!()
    }


}
