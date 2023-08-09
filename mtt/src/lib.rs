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
    essential::{GameEvent, GameMode, HoldemStage, InternalPlayerJoin, Player},
    game::Holdem,
};
use race_proc_macro::game_handler;

fn error_player_not_found() -> HandleError {
    HandleError::Custom("Player not found".to_string())
}

/// Following errors are internal errors
fn error_table_not_fonud() -> HandleError {
    HandleError::Custom("Table not found".to_string())
}
fn error_table_is_empty() -> HandleError {
    HandleError::Custom("Table is empty".to_string())
}
fn error_empty_blind_rules() -> HandleError {
    HandleError::Custom("Empty blind rules".to_string())
}

pub type TableId = u8;

const STARTING_SB: u64 = 50;
const STARTING_BB: u64 = 100;

#[derive(BorshSerialize, BorshDeserialize, PartialEq, Eq, Default)]
pub enum MttStage {
    #[default]
    Init,
    Playing,
    Completed,
}

#[derive(BorshSerialize, BorshDeserialize, PartialEq, Eq, Default)]
pub enum PlayerRankStatus {
    #[default]
    Alive,
    Out,
}

#[derive(BorshSerialize, BorshDeserialize)]
pub struct PlayerRank {
    addr: String,
    balance: u64,
    status: PlayerRankStatus,
}

#[derive(BorshSerialize, BorshDeserialize)]
pub struct BlindRuleItem {
    sb_x: u16,
    bb_x: u16,
}

impl BlindRuleItem {
    fn new(sb_x: u16, bb_x: u16) -> Self {
        Self { sb_x, bb_x }
    }
}

fn default_blind_rules() -> Vec<BlindRuleItem> {
    [(2, 3), (3, 5), (4, 6), (8, 12), (12, 16), (16, 20)]
        .into_iter()
        .map(|(sb, bb)| BlindRuleItem::new(sb, bb))
        .collect()
}

#[allow(unused)]
#[derive(Default)]
enum ReseatMethod {
    #[default]
    Noop,

    /// This table has the least players, if there are enough empty
    /// seats in other tables, close this table and move its players
    /// to other tables
    CloseTable { close_table_id: TableId },
    /// This table has the most players, if the number of players on
    /// this table is greater than the number of the table with least
    /// players, move one player to that table
    MovePlayer {
        from_table_id: TableId,
        to_table_id: TableId,
    },
}

#[derive(Default, BorshSerialize, BorshDeserialize)]
pub struct MttProperties {
    starting_chips: u64,
    start_time: u64,
    table_size: u8,
    blind_base: u64,
    blind_interval: u64,
    blind_rules: Vec<BlindRuleItem>,
}

#[game_handler]
#[derive(BorshSerialize, BorshDeserialize, Default)]
pub struct Mtt {
    // The number of alive players
    alives: usize,
    stage: MttStage,
    // The mapping from player addresses to table IDs
    table_assigns: BTreeMap<String, TableId>,
    // All players in rank order, including eliminated players
    ranks: Vec<PlayerRank>,
    // The mapping from table IDs to game states
    games: BTreeMap<TableId, Holdem>,
    // Must be between 2 and 9
    table_size: u8,
    // Inherited from properties
    starting_chips: u64,
    // How much time spend so far. Usually it should match current time - start time,
    // unless the game was interrupted in the middle
    time_spend: u64,
    timestamp: u64,
    // The minimal blind unit, used to calculate blinds structure
    blind_base: u64,
    // Blind rules
    blind_interval: u64,
    blind_rules: Vec<BlindRuleItem>,
}

impl GameHandler for Mtt {
    fn init_state(effect: &mut Effect, init_account: InitAccount) -> Result<Self, HandleError> {
        let props = MttProperties::try_from_slice(&init_account.data)
            .map_err(|_| HandleError::MalformedGameAccountData)?;
        let mut ranks: Vec<PlayerRank> = Vec::default();

        for p in init_account.players {
            let status = if p.balance == 0 {
                PlayerRankStatus::Out
            } else {
                PlayerRankStatus::Alive
            };
            ranks.push(PlayerRank {
                addr: p.addr,
                balance: p.balance,
                status,
            });
        }

        // Unregister is not supported for now
        effect.allow_exit(false);

        // Schedule the startup
        effect.wait_timeout(props.start_time - effect.timestamp);

        Ok(Self {
            ranks,
            timestamp: effect.timestamp(),
            table_size: props.table_size,
            starting_chips: props.starting_chips,
            blind_base: props.blind_base,
            blind_interval: props.blind_interval,
            blind_rules: if props.blind_rules.is_empty() {
                default_blind_rules()
            } else {
                props.blind_rules
            },
            ..Default::default()
        })
    }

    fn handle_event(&mut self, effect: &mut Effect, event: Event) -> Result<(), HandleError> {
        // Update time spend for blinds calculation.
        self.time_spend = effect.timestamp() - self.timestamp;
        self.timestamp = effect.timestamp();

        let mut updated_table_ids: Vec<TableId> = Vec::with_capacity(5);

        match event {
            // Delegate the custom events to sub event handlers
            Event::Custom { sender, raw } => {
                if let Some(table_id) = self.table_assigns.get(&sender) {
                    let game = self
                        .games
                        .get_mut(table_id)
                        .ok_or(error_table_not_fonud())?;
                    let event = GameEvent::try_parse(&raw)?;
                    game.handle_custom_event(effect, event, sender)?;
                } else {
                    return Err(HandleError::InvalidPlayer);
                }
            }

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
                            status: PlayerRankStatus::Alive,
                        })
                    }
                } else {
                    for p in new_players {
                        effect.settle(Settle::eject(p.addr));
                    }
                }
            }
            Event::GameStart { access_version } => {
                self.create_tables()?;
                self.stage = MttStage::Playing;
                for game in self.games.values_mut() {
                    if game.player_map.len() > 1 {
                        game.handle_event(effect, Event::GameStart { access_version })?;
                    }
                }
            }

            // In Mtt, there's only one WaitingTimeout event.
            // We should start the game in this case.
            Event::WaitingTimeout => {
                effect.start_game();
            }

            // Delegate to the player's table
            Event::ActionTimeout { ref player_addr } => {
                let table_id = self
                    .table_assigns
                    .get(player_addr)
                    .ok_or(error_player_not_found())?;
                let game = self
                    .games
                    .get_mut(table_id)
                    .ok_or(error_table_not_fonud())?;
                game.handle_event(effect, event)?;
                if game.stage == HoldemStage::Init {
                    updated_table_ids.push(*table_id);
                }
            }

            Event::SecretsReady { ref random_ids } => {
                for random_id in random_ids {
                    let games = self.games.values_mut();
                    for game in games {
                        if game.deck_random_id == *random_id {
                            game.handle_event(effect, event.clone())?;
                        }
                    }
                }
            }
            _ => (),
        }

        for table_id in updated_table_ids {
            self.update_rank_balance(table_id)?;
            self.maybe_raise_blinds(table_id)?;
            self.maybe_reseat_players(effect, table_id)?;
        }
        self.sort_ranks();

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
                mode: GameMode::Mtt,
                ..Default::default()
            };
            self.games.insert(i, game);
        }

        Ok(())
    }

    fn update_rank_balance(&mut self, table_id: TableId) -> Result<(), HandleError> {
        let game = self.games.get(&table_id).ok_or(error_table_not_fonud())?;
        for (addr, p) in game.player_map.iter() {
            let rank = self
                .ranks
                .iter_mut()
                .find(|p| p.addr.eq(addr))
                .ok_or(error_player_not_found())?;
            rank.balance = p.chips;
        }
        Ok(())
    }

    fn sort_ranks(&mut self) {
        self.ranks.sort_by(|r1, r2| r2.balance.cmp(&r1.balance));
    }

    fn maybe_raise_blinds(&mut self, table_id: TableId) -> Result<(), HandleError> {
        let time_spend = self.time_spend;
        let level = time_spend / self.blind_interval;
        let mut blind_rule = self.blind_rules.get(level as usize);
        if blind_rule.is_none() {
            blind_rule = self.blind_rules.last();
        }
        let blind_rule = blind_rule.ok_or(error_empty_blind_rules())?;
        let sb = blind_rule.sb_x as u64 * self.blind_base;
        let bb = blind_rule.bb_x as u64 * self.blind_base;
        let game = self
            .games
            .get_mut(&table_id)
            .ok_or(error_table_not_fonud())?;
        game.sb = sb;
        game.bb = bb;
        Ok(())
    }

    fn maybe_reseat_players(
        &mut self,
        _effect: &mut Effect,
        table_id: TableId,
    ) -> Result<(), HandleError> {
        // No-op for final table
        if self.games.len() == 1 {
            return Ok(());
        }

        let table_size = self.table_size as usize;

        let reseat_method = {
            let Some(first_table) = self.games.first_key_value() else {
            return Ok(())
        };

            let mut table_with_least = first_table;
            let mut table_with_most = first_table;

            for (id, game) in self.games.iter() {
                if game.player_map.len() < table_with_least.1.player_map.len() {
                    table_with_least = (id, game);
                }
                if game.player_map.len() > table_with_most.1.player_map.len() {
                    table_with_most = (id, game);
                }
            }
            let total_empty_seats = self
                .games
                .iter()
                .map(|(id, g)| {
                    if id == table_with_least.0 {
                        0
                    } else {
                        table_size - g.player_map.len()
                    }
                })
                .sum::<usize>();
            if table_id == *table_with_least.0
                && table_with_least.1.player_map.len() <= total_empty_seats
            {
                ReseatMethod::CloseTable {
                    close_table_id: table_id,
                }
            } else if table_id == *table_with_most.0
                && table_with_most.1.player_map.len() > table_with_least.1.player_map.len() + 1
            {
                ReseatMethod::MovePlayer {
                    from_table_id: table_id,
                    to_table_id: *table_with_least.0,
                }
            } else {
                ReseatMethod::Noop
            }
        };

        match reseat_method {
            ReseatMethod::Noop => (),
            ReseatMethod::CloseTable { close_table_id } => {
                // Remove this game
                let mut game_to_close = self
                    .games
                    .remove(&close_table_id)
                    .ok_or(error_table_not_fonud())?;

                // Iterate all other games, move player if there're empty
                // seats available.  The iteration should be sorted by
                // game's player numbers in ascending order
                let mut game_refs = self
                    .games
                    .iter_mut()
                    .collect::<Vec<(&TableId, &mut Holdem)>>();

                game_refs.sort_by_key(|(_id, g)| g.player_map.len());

                for (id, game_ref) in game_refs {
                    let cnt = table_size - game_ref.player_map.len();
                    let mut moved_players = Vec::with_capacity(cnt);
                    for _ in 0..cnt {
                        if let Some((player_addr, player)) = game_to_close.player_map.pop_first() {
                            moved_players.push(InternalPlayerJoin {
                                addr: player.addr,
                                chips: player.chips,
                            });
                            self.table_assigns.insert(player_addr, *id);
                        } else {
                            break;
                        }
                    }

                    game_ref.internal_add_players(moved_players)?;

                    if game_to_close.player_map.is_empty() {
                        break;
                    }
                }
            }
            ReseatMethod::MovePlayer {
                from_table_id,
                to_table_id,
            } => {
                let from_table = self
                    .games
                    .get_mut(&from_table_id)
                    .ok_or(error_table_not_fonud())?;
                let (addr, p) = from_table
                    .player_map
                    .pop_first()
                    .ok_or(error_table_is_empty())?;
                let add_players = vec![InternalPlayerJoin {
                    addr: p.addr,
                    chips: p.chips,
                }];
                let to_table = self
                    .games
                    .get_mut(&to_table_id)
                    .ok_or(error_player_not_found())?;
                to_table.internal_add_players(add_players)?;
                self.table_assigns.insert(addr, to_table_id);
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {

    use race_core::context::GameContext;
    use race_test::{sync_new_players, TestClient, TestGameAccountBuilder, TestHandler};

    use super::*;

    #[test]
    pub fn test_new_player_join() -> anyhow::Result<()> {
        let mut effect = Effect::default();
        let mut handler = Mtt::default();
        assert_eq!(handler.ranks.len(), 0);
        let event = sync_new_players(&[("alice", 0, 1000)], 1);
        handler.handle_event(&mut effect, event)?;
        assert_eq!(handler.ranks.len(), 1);
        Ok(())
    }

    #[test]
    pub fn test_game_start() -> anyhow::Result<()> {
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
        handler.handle_event(&mut effect, event)?;
        let event = Event::GameStart { access_version: 1 };
        handler.handle_event(&mut effect, event)?;
        assert_eq!(handler.ranks.len(), 5);
        assert_eq!(handler.games.len(), 3);
        Ok(())
    }

    pub fn create_sync_event(
        ctx: &GameContext,
        players: &[&TestClient],
        transactor: &TestClient,
    ) -> Event {
        let av = ctx.get_access_version() + 1;
        let max_players = 9usize;
        let used_pos: Vec<usize> = ctx.get_players().iter().map(|p| p.position).collect();
        let mut new_players = Vec::new();
        for (i, p) in players.iter().enumerate() {
            let mut position = i;
            // If a position already taken, append the new player to the last
            if used_pos.get(position).is_some() {
                if position + 1 < max_players {
                    position = ctx.count_players() as usize + 1;
                } else {
                    println!("Game is full");
                }
            }
            new_players.push(PlayerJoin {
                addr: p.get_addr(),
                balance: 10_000,
                position: position as u16,
                access_version: av,
                verify_key: p.get_addr(),
            })
        }

        Event::Sync {
            new_players,
            new_servers: vec![],
            transactor_addr: transactor.get_addr(),
            access_version: av,
        }
    }

    #[test]
    pub fn integration_test() -> anyhow::Result<()> {
        let mtt_props = MttProperties {
            table_size: 2,
            starting_chips: 1000,
            blind_base: 50,
            blind_interval: 10000000,
            start_time: 0,
            ..Default::default()
        };

        let mut transactor = TestClient::transactor("Tx");
        let game_account = TestGameAccountBuilder::default()
            .with_data(mtt_props)
            .set_transactor(&transactor)
            .build();
        let mut ctx = GameContext::try_new(&game_account).unwrap();
        let mut handler = TestHandler::<Mtt>::init_state(&mut ctx, &game_account).unwrap();

        let mut alice = TestClient::player("Alice");
        let mut bob = TestClient::player("Bob");
        let mut carol = TestClient::player("Carol");
        let mut dave = TestClient::player("Dave");
        let mut eva = TestClient::player("Eva");

        let sync_evt = create_sync_event(&ctx, &[&alice, &bob, &carol, &dave, &eva], &transactor);

        handler.handle_until_no_events(&mut ctx, &sync_evt, vec![&mut transactor])?;

        {
            let state = handler.get_state();
            assert_eq!(state.ranks.len(), 5);
        }

        let wait_timeout_evt = Event::WaitingTimeout;

        handler.handle_until_no_events(&mut ctx, &wait_timeout_evt, vec![&mut transactor])?;

        {
            let state = handler.get_state();
            assert_eq!(state.games.len(), 3);
            let (_, game0) = state.games.first_key_value().unwrap();
            assert_eq!(game0.player_map.len(), 2);
            assert_eq!(game0.stage, HoldemStage::Play);
            // Table assigns:
            // 1. Alice, Dave
            // 2. Bob, Eva
            // 3. Carol
        }

        // Dave allin
        let dave_call = dave.custom_event(GameEvent::Raise(1000));
        handler.handle_until_no_events(&mut ctx, &dave_call, vec![&mut transactor])?;

        {
            let state = handler.get_state();
        }

        Ok(())
    }
}
