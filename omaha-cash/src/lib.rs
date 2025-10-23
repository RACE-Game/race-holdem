use race_api::prelude::*;
use race_poker_base::game::PokerGame;
use race_poker_base::omaha::OmahaVariant;
use race_proc_macro::game_handler;

#[game_handler]
#[derive(BorshSerialize, BorshDeserialize)]
pub struct OmahaCash(PokerGame<OmahaVariant>);

impl GameHandler for OmahaCash {
    fn balances(&self) -> Vec<PlayerBalance> {
        self.0.balances()
    }

    fn init_state(effect: &mut Effect, init_account: InitAccount) -> Result<Self, HandleError> {
        Ok(Self(PokerGame::<OmahaVariant>::init_state(effect, init_account)?))
    }

    fn handle_event(&mut self, effect: &mut Effect, event: Event) -> Result<(), HandleError> {
        self.0.handle_event(effect, event)
    }
}
