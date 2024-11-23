mod errors;

use borsh::{BorshDeserialize, BorshSerialize};
use race_api::event::BridgeEvent;
use race_api::prelude::*;
use race_holdem_base::essential::{GameMode, HoldemStage, Player, PlayerStatus};
use race_holdem_base::game::Holdem;
use race_holdem_mtt_base::{ChipsChange, HoldemBridgeEvent, MttTablePlayer, MttTableState};
use race_proc_macro::game_handler;

pub type PlayerId = u64;

#[game_handler]
#[derive(Debug, BorshSerialize, BorshDeserialize, Default, Clone)]
pub struct MttTable {
    pub table_id: u8,
    pub hand_id: usize,
    pub holdem: Holdem,
}

impl GameHandler for MttTable {
    fn init_state(init_account: InitAccount) -> HandleResult<Self> {

        let MttTableState { sb, bb, players, table_id, btn, .. } = init_account.data()?;

        let player_map = players.into_iter().map(|p| (p.id, Player::new(p.id, p.chips, p.table_position as _, 0))).collect();

        let holdem = Holdem {
            btn,
            sb,
            bb,
            table_size: init_account.max_players as _,
            mode: GameMode::Mtt,
            player_map,
            ..Default::default()
        };

        Ok(Self {
            table_id,
            hand_id: 0,
            holdem,
        })
    }

    fn handle_event(&mut self, effect: &mut Effect, event: Event) -> HandleResult<()> {
        match event {
            Event::Bridge {
                dest_game_id,
                raw,
            } => {
                if dest_game_id == self.table_id as _ {
                    let bridge_event = HoldemBridgeEvent::try_parse(&raw)?;
                    self.handle_bridge_event(effect, bridge_event)?;
                }
            }
            _ => {
                self.holdem.handle_event(effect, event)?;
                // Check if there's a settlement
                if effect.is_checkpoint() {
                    let players = self
                        .holdem
                        .player_map
                        .values()
                        .map(|p| MttTablePlayer::new(p.id, p.chips, p.position as _))
                        .collect();
                    let mtt_table_state = MttTableState {
                        table_id: self.table_id,
                        btn: self.holdem.btn,
                        hand_id: self.hand_id,
                        sb: self.holdem.sb,
                        bb: self.holdem.bb,
                        next_game_start: self.holdem.next_game_start,
                        players
                    };
                    let chips_change = self.holdem.hand_history.chips_change.iter().filter_map(|(pid, change)| {
                        match change {
                            race_holdem_base::hand_history::ChipsChange::NoUpdate => None,
                            race_holdem_base::hand_history::ChipsChange::Add(amt) => Some((*pid, ChipsChange::Add(*amt))),
                            race_holdem_base::hand_history::ChipsChange::Sub(amt) => Some((*pid, ChipsChange::Sub(*amt))),
                        }
                    }).collect();
                    let evt = HoldemBridgeEvent::GameResult {
                        hand_id: self.hand_id,
                        table: mtt_table_state,
                        chips_change,
                        table_id: self.table_id,
                    };
                    effect.checkpoint();
                    effect.bridge_event(0, evt)?;
                }
            }
        };

        Ok(())
    }
}

impl MttTable {
    fn handle_bridge_event(
        &mut self,
        effect: &mut Effect,
        event: HoldemBridgeEvent,
    ) -> HandleResult<()> {
        match event {
            HoldemBridgeEvent::StartGame {
                sb,
                bb,
                moved_players,
            } => {
                let timeout = self
                    .holdem
                    .next_game_start
                    .saturating_sub(effect.timestamp());
                self.holdem.reset_state()?;
                self.holdem.sb = sb;
                self.holdem.bb = bb;
                for id in moved_players {
                    self.holdem.player_map.remove(&id);
                }
                effect.wait_timeout(timeout);
            }
            // Add players from other tables
            HoldemBridgeEvent::Relocate { players } => {
                for mtt_player in players.into_iter() {
                    let MttTablePlayer {
                        id,
                        chips,
                        table_position,
                    } = mtt_player;
                    self.holdem.player_map.insert(
                        id,
                        Player::new_with_status(id, chips, table_position as _, PlayerStatus::Init),
                    );
                }
                if matches!(self.holdem.stage, HoldemStage::Init | HoldemStage::Settle | HoldemStage::Runner) {
                    let timeout = self
                        .holdem
                        .next_game_start
                        .saturating_sub(effect.timestamp());
                    effect.wait_timeout(timeout);
                }
            }
            HoldemBridgeEvent::CloseTable => {
                self.holdem.player_map.clear();
                effect.checkpoint();
            }
            _ => return Err(errors::internal_invalid_bridge_event()),
        };
        Ok(())
    }
}

#[cfg(test)]
mod tests {

    use super::*;


    #[test]
    fn a() -> anyhow::Result<()> {
        let effect = vec![0,0,0,0,0,133,223,135,73,147,1,0,0,1,0,0,0,0,0,0,0,1,0,0,0,0,0,0,0,3,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,1,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0];
        let event = vec![19,1,0,0,0,0,0,0,0,21,0,0,0,0,100,0,0,0,0,0,0,0,200,0,0,0,0,0,0,0,0,0,0,0];

        let mut effect = Effect::try_from_slice(&effect)?;
        let event = Event::try_from_slice(&event)?;

        println!("1");

        let st = effect.handler_state.clone().unwrap();
        println!("effect: {:?}", effect);

        println!("2");
        println!("st: {:?}", st);

        let mut st = MttTable::try_from_slice(&st)?;

        println!("3");
        st.handle_event(&mut effect, event)?;

        println!("4");
        Ok(())
    }
}
