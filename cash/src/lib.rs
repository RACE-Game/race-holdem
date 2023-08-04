use race_core::prelude::*;
use race_holdem_base::game::Holdem;

#[derive(BorshSerialize, BorshDeserialize)]
#[game_handler]
pub struct Cash(Holdem);

impl GameHandler for Cash {
    fn init_state(effect: &mut Effect, init_account: InitAccount) -> Result<Self, HandleError> {
        Ok(Self (Holdem::init_state(effect, init_account)?))
    }

    fn handle_event(&mut self, effect: &mut Effect, event: Event) -> Result<(), HandleError> {
        self.0.handle_event(effect, event)
    }
}
