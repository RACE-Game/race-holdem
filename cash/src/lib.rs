use race_api::prelude::*;
use race_holdem_base::game::{Holdem, HoldemCheckpoint};
use race_proc_macro::game_handler;

#[derive(BorshSerialize, BorshDeserialize)]
#[game_handler]
pub struct Cash(Holdem);

#[derive(Debug, BorshSerialize, BorshDeserialize)]
pub struct CashCheckpoint(HoldemCheckpoint);

impl GameHandler for Cash {
    fn init_state(effect: &mut Effect, init_account: InitAccount) -> Result<Self, HandleError> {
        Ok(Self(Holdem::init_state(effect, init_account)?))
    }

    fn handle_event(&mut self, effect: &mut Effect, event: Event) -> Result<(), HandleError> {
        self.0.handle_event(effect, event)
    }
}

#[cfg(test)]
mod tests {
    use borsh::BorshDeserialize;

    use crate::CashCheckpoint;

    #[test]
    fn test_checkpoint() {
        let cp_data = vec![
            1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 56, 0, 0, 0, 0, 0, 0, 0, 38, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 2, 0, 0, 0, 19, 0, 0, 0, 0, 0, 0, 0, 1, 20, 0, 0, 0, 0, 0, 0, 0, 1, 162,
            136, 186, 47, 144, 1, 0, 0,
        ];
        let cp = CashCheckpoint::try_from_slice(&cp_data).unwrap();
        println!("checkpoint: {:?}", cp);
    }
}
