//! # Holdem MTT
//!
//! # Stages
//!
//! There are three stages in the whole game progress:
//! - Init, the initial state, players can buy-in.
//! - Playing, the game is in progress.
//! - Completed, the game is finished.
//!
//! ## Game start
//!
//! The game will start at `start-time`, saved in the account data.
//! Depends on the number of players and the table size, some tables
//! will be created.  The same data structure with cash table is used
//! for each table in the tournament.
//!

use std::collections::BTreeMap;

use borsh::{BorshDeserialize, BorshSerialize};
use race_core::prelude::*;
use race_holdem_base::{
    essential::{GameEvent, HoldemStage, Player, Street},
    game::Holdem,
};

fn error_table_not_fonud() -> Result<(), HandleError> {
    Err(HandleError::Custom("Table not found".to_string()))
}

const STARTING_SB: u64 = 50;
const STARTING_BB: u64 = 100;

#[derive(BorshSerialize, BorshDeserialize, PartialEq, Eq, Default)]
pub enum MttStage {
    #[default]
    Init,
    Playing,
    Completed,
}

#[derive(BorshSerialize, BorshDeserialize)]
pub struct PlayerRank {
    addr: String,
    balance: u64,
}

#[derive(BorshSerialize, BorshDeserialize)]
pub struct MttProperties {
    starting_chips: u64,
    start_time: u64,
    table_size: u8,
}

#[game_handler]
#[derive(BorshSerialize, BorshDeserialize, Default)]
pub struct Mtt {
    stage: MttStage,
    // The mapping from player addresses to table IDs
    table_assigns: BTreeMap<String, u8>,
    // All players in rank order, including eliminated players
    ranks: Vec<PlayerRank>,
    // The mapping from table IDs to game states
    games: BTreeMap<u8, Holdem>,
    // Must be between 2 and 9
    table_size: u8,
    // Inherited from properties
    starting_chips: u64,
}

impl GameHandler for Mtt {
    fn init_state(effect: &mut Effect, init_account: InitAccount) -> Result<Self, HandleError> {
        let props = MttProperties::try_from_slice(&init_account.data)
            .map_err(|_| HandleError::MalformedGameAccountData)?;
        let mut ranks: Vec<PlayerRank> = Vec::default();

        for p in init_account.players {
            ranks.push(PlayerRank {
                addr: p.addr,
                balance: p.balance,
            });
        }

        effect.allow_exit(false);
        effect.wait_timeout(props.start_time - effect.timestamp);
        Ok(Self {
            ranks,
            ..Default::default()
        })
    }

    fn handle_event(&mut self, effect: &mut Effect, event: Event) -> Result<(), HandleError> {
        match event {
            // Delegate the custom events to sub event handlers
            Event::Custom { sender, raw } => {
                if let Some(table_id) = self.table_assigns.get(&sender) {
                    if let Some(game) = self.games.get_mut(table_id) {
                        let event = GameEvent::try_parse(&raw)?;
                        game.handle_custom_event(effect, event, sender)?;
                    } else {
                        return error_table_not_fonud();
                    }
                } else {
                    return Err(HandleError::InvalidPlayer);
                }
            }
            Event::Ready => todo!(),
            Event::ShareSecrets { sender, shares } => todo!(),
            Event::OperationTimeout { addrs } => todo!(),

            // Delegate to the corresponding game
            Event::RandomnessReady { random_id } => {
                if let Some(game) = self
                    .games
                    .values_mut()
                    .find(|g| g.deck_random_id == random_id)
                {
                    game.handle_event(effect, event)?
                }
            }

            // Add current event to ranks
            // Reject buy-in if the game is already started
            Event::Sync { new_players, .. } => {
                if self.stage == MttStage::Init {
                    for p in new_players {
                        self.ranks.push(PlayerRank {
                            addr: p.addr,
                            balance: p.balance,
                        })
                    }
                } else {
                    for p in new_players {
                        effect.settle(Settle::eject(p.addr));
                    }
                }
            }
            Event::GameStart { .. } => {
                self.create_tables()?;
                self.stage = MttStage::Playing;
            }

            // In Mtt, there's only one WaitingTimeout event.
            // We should start the game in this case.
            Event::WaitingTimeout => {
                effect.start_game();
            }

            // Delegate to the player's table
            Event::ActionTimeout { ref player_addr } => {
                if let Some(table_id) = self.table_assigns.get(player_addr) {
                    if let Some(game) = self.games.get_mut(table_id) {
                        game.handle_event(effect, event)?;
                    } else {
                        return error_table_not_fonud();
                    }
                }
            }
            Event::SecretsReady => {}
            _ => (),
        }
        Ok(())
    }
}

impl Mtt {
    fn create_tables(&mut self) -> Result<(), HandleError> {
        let num_of_players = self.ranks.len();
        let num_of_tables = (self.table_size + num_of_players as u8 - 1) / self.table_size;
        for i in 0..num_of_tables {
            let mut player_map = BTreeMap::<String, Player>::default();
            let mut j = i;
            while let Some(r) = self.ranks.get(j as usize) {
                player_map.insert(
                    r.addr.to_owned(),
                    Player::new(
                        r.addr.to_owned(),
                        self.starting_chips,
                        (j / num_of_tables) as u16,
                    ),
                );
                j += num_of_tables;
            }
            let game = Holdem {
                sb: STARTING_SB,
                bb: STARTING_BB,
                player_map,
                ..Default::default()
            };
            self.games.insert(i, game);
        }

        Ok(())
    }

    fn maybe_raise_blinds(&mut self) -> Result<(), HandleError> {
        Ok(())
    }

    fn maybe_reseat_players(&mut self) -> Result<(), HandleError> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {

    use super::*;

    type NewPlayerSpec<'a> = (&'a str, u16, u64);

    pub fn sync_new_players(addr_pos_balance_list: &[NewPlayerSpec], access_version: u64) -> Event {
        let mut new_players: Vec<PlayerJoin> = Vec::default();

        for (addr, pos, balance) in addr_pos_balance_list.iter() {
            if new_players.iter().find(|p| p.addr.eq(addr)).is_some() {
                panic!("Duplicated address: {}", addr)
            }
            if *balance == 0 {
                panic!("Zero balance: {}", addr)
            }
            if new_players.iter().find(|p| p.position.eq(pos)).is_some() {
                panic!("Duplicated position: {} at {}", addr, pos)
            }
            new_players.push(PlayerJoin {
                addr: addr.to_string(),
                position: *pos,
                balance: *balance,
                verify_key: "".to_string(),
                access_version,
            });
        }

        Event::Sync {
            new_players,
            new_servers: Default::default(),
            transactor_addr: "".to_string(),
            access_version,
        }
    }

    #[test]
    pub fn test_new_player_join() {
        let mut effect = Effect::default();
        let mut handler = Mtt::default();
        assert_eq!(handler.ranks.len(), 0);
        let event = sync_new_players(&[("alice", 0, 1000)], 1);
        handler.handle_event(&mut effect, event).unwrap();
        assert_eq!(handler.ranks.len(), 1);
    }

    #[test]
    pub fn test_game_start() {
        let mut effect = Effect::default();
        let mut handler = Mtt::default();
        handler.table_size = 2;
        // Add 5 players
        let event = sync_new_players(
            &[
                ("p1", 0, 1000),
                ("p2", 1, 1000),
                ("p3", 2, 1000),
                ("p4", 3, 1000),
                ("p5", 4, 1000),
            ],
            1,
        );
        handler.handle_event(&mut effect, event).unwrap();
        let event = Event::GameStart { access_version: 1 };
        handler.handle_event(&mut effect, event).unwrap();
        assert_eq!(handler.ranks.len(), 5);
        assert_eq!(handler.games.len(), 3);
    }
}
