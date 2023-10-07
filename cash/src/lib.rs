use race_api::prelude::*;
use race_holdem_base::game::Holdem;
use race_proc_macro::game_handler;

#[derive(BorshSerialize, BorshDeserialize)]
#[game_handler]
pub struct Cash(Holdem);

#[derive(BorshSerialize, BorshDeserialize)]
pub struct CashCheckpoint {
    btn: usize,
}

impl GameHandler for Cash {
    type Checkpoint = CashCheckpoint;

    fn init_state(effect: &mut Effect, init_account: InitAccount) -> Result<Self, HandleError> {
        Ok(Self (Holdem::init_state(effect, init_account)?))
    }

    fn handle_event(&mut self, effect: &mut Effect, event: Event) -> Result<(), HandleError> {
        self.0.handle_event(effect, event)
    }

    fn into_checkpoint(self) -> Result<CashCheckpoint, HandleError> {
        Ok(CashCheckpoint { btn: self.0.btn })
    }
}
