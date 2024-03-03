use race_api::prelude::*;
use race_holdem_base::game::{Holdem, HoldemCheckpoint};
use race_proc_macro::game_handler;

#[derive(BorshSerialize, BorshDeserialize)]
#[game_handler]
pub struct Cash(Holdem);

#[derive(BorshSerialize, BorshDeserialize)]
pub struct CashCheckpoint(HoldemCheckpoint);

impl GameHandler for Cash {
    fn init_state(effect: &mut Effect, init_account: InitAccount) -> Result<Self, HandleError> {
        Ok(Self (Holdem::init_state(effect, init_account)?))
    }

    fn handle_event(&mut self, effect: &mut Effect, event: Event) -> Result<(), HandleError> {
        self.0.handle_event(effect, event)
    }
}
