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
//! will be created.  The same data structure as in cash table is used
//! for each table in the tournament.
//!

mod errors;

use std::collections::BTreeMap;

use borsh::{BorshDeserialize, BorshSerialize};
use race_api::{prelude::*, types::SettleOp};
use race_holdem_base::{
    essential::{ActingPlayer, GameEvent, GameMode, HoldemStage, InternalPlayerJoin, Player},
    game::Holdem,
};
use race_proc_macro::game_handler;

#[cfg(test)]
mod tests;

pub type TableId = u8;

#[derive(BorshSerialize, BorshDeserialize, PartialEq, Eq, Default, Debug)]
pub enum MttStage {
    #[default]
    Init,
    Playing,
    Completed,
}

#[derive(Debug, BorshSerialize, BorshDeserialize, PartialEq, Eq, Default)]
pub enum PlayerRankStatus {
    #[default]
    Alive,
    Out,
}

#[derive(Debug, BorshSerialize, BorshDeserialize, PartialEq, Eq)]
pub struct PlayerRank {
    addr: String,
    chips: u64,
    status: PlayerRankStatus,
}

impl PlayerRank {
    pub fn new<S: Into<String>>(addr: S, chips: u64, status: PlayerRankStatus) -> Self {
        Self {
            addr: addr.into(),
            chips,
            status,
        }
    }
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
    [(1, 2), (2, 3), (3, 6), (8, 12), (12, 16), (16, 20)]
        .into_iter()
        .map(|(sb, bb)| BlindRuleItem::new(sb, bb))
        .collect()
}

#[derive(Default, Debug, BorshDeserialize, BorshSerialize, Clone)]
enum TableUpdate {
    /// The last event affects this table, but no player has to be
    /// moved.
    #[default]
    Noop,
    /// This table has the least players. If there are enough empty
    /// seats at other tables, close this table and move its players
    /// to other tables
    CloseTable { close_table_id: TableId },
    /// This table has the most players. If the number of players at
    /// this table is greater than that of the table with least
    /// players, move one player to that table
    MovePlayer {
        from_table_id: TableId,
        to_table_id: TableId,
    },
}

#[derive(Default, BorshSerialize, BorshDeserialize)]
pub struct MttProperties {
    start_time: u64,
    table_size: u8,
    blind_base: u64,
    blind_interval: u64,
    blind_rules: Vec<BlindRuleItem>,
}

#[game_handler]
#[derive(BorshSerialize, BorshDeserialize, Default)]
pub struct Mtt {
    start_time: u64,
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
    // How much time spend so far. Usually it should match current time - start time,
    // unless the game was interrupted in the middle
    time_spend: u64,
    timestamp: u64,
    // The minimal blind unit, used to calculate blinds structure
    blind_base: u64,
    // Blind rules
    blind_interval: u64,
    blind_rules: Vec<BlindRuleItem>,
    // Table updates in this event
    table_updates: BTreeMap<TableId, TableUpdate>,
}

#[derive(BorshSerialize, BorshDeserialize)]
pub struct MttCheckpoint {}

impl GameHandler for Mtt {
    type Checkpoint = MttCheckpoint;

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
            ranks.push(PlayerRank::new(p.addr, p.balance, status));
        }

        // Unregister is not supported for now
        effect.allow_exit(false);

        Ok(Self {
            start_time: props.start_time,
            ranks,
            timestamp: effect.timestamp(),
            table_size: props.table_size,
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
        self.time_spend = self.time_spend + effect.timestamp() - self.timestamp;
        self.timestamp = effect.timestamp();
        self.table_updates.clear();

        let mut updated_table_ids: Vec<TableId> = Vec::with_capacity(5);
        let mut no_timeout = false; // We try to dispatch ActionTimeout by default

        match event {
            Event::Ready => {
                // Schedule the startup
                if self.start_time > effect.timestamp {
                    effect.wait_timeout(self.start_time - effect.timestamp);
                } else {
                    effect.wait_timeout(0);
                }
            }

            // XXX
            // EventLoop will dispatch SecretsReady event which can be overwritten by our action timeout
            Event::ShareSecrets { .. } => {
                no_timeout = true;
            }

            // Delegate the custom events to sub event handlers
            // Single table update
            Event::Custom { sender, raw } => {
                if let Some(table_id) = self.table_assigns.get(&sender) {
                    let game = self
                        .games
                        .get_mut(table_id)
                        .ok_or(errors::error_table_not_fonud())?;
                    let event = GameEvent::try_parse(&raw)?;
                    game.handle_custom_event(effect, event, sender)?;
                    updated_table_ids.push(*table_id);
                } else {
                    return Err(HandleError::InvalidPlayer);
                }
            }

            // Delegate to the corresponding game
            // Single table update
            Event::RandomnessReady { random_id } => {
                if let Some((table_id, game)) = self
                    .games
                    .iter_mut()
                    .find(|(_, g)| g.deck_random_id == random_id)
                {
                    game.handle_event(effect, event)?;
                    updated_table_ids.push(*table_id);
                }
            }

            // Add current event to ranks
            // Reject buy-in if the game is already started
            Event::Sync { new_players, .. } => {
                if self.stage == MttStage::Init {
                    for p in new_players {
                        self.ranks.push(PlayerRank::new(
                            p.addr,
                            p.balance,
                            PlayerRankStatus::Alive,
                        ));
                    }
                }
            }

            Event::GameStart { .. } => {
                self.create_tables()?;
                self.stage = MttStage::Playing;
                for game in self.games.values_mut() {
                    if game.player_map.len() > 1 {
                        game.reset_holdem_state()?;
                        game.reset_player_map_status()?;
                        game.internal_start_game(effect)?;
                    }
                }
            }

            // The first is used to start game
            // The rest are for sub game start
            // Multiple table updates
            Event::WaitingTimeout => match self.stage {
                MttStage::Init => {
                    effect.start_game();
                }
                MttStage::Playing => {
                    for (table_id, game) in self.games.iter_mut() {
                        if game.next_game_start != 0 && game.next_game_start <= effect.timestamp() {
                            game.reset_holdem_state()?;
                            game.reset_player_map_status()?;
                            game.internal_start_game(effect)?;
                            updated_table_ids.push(*table_id);
                        }
                    }
                }
                _ => (),
            },

            // Delegate to the player's table
            // Single table update
            Event::ActionTimeout { ref player_addr } => {
                let table_id = self
                    .table_assigns
                    .get(player_addr)
                    .ok_or(errors::error_player_not_found())?;
                let game = self
                    .games
                    .get_mut(table_id)
                    .ok_or(errors::error_table_not_fonud())?;
                game.handle_event(effect, event)?;
                updated_table_ids.push(*table_id);
            }

            // Multiple table updates
            Event::SecretsReady { ref random_ids } => {
                for random_id in random_ids {
                    for (id, game) in self.games.iter_mut() {
                        if game.deck_random_id == *random_id {
                            game.handle_event(effect, event.clone())?;
                            updated_table_ids.push(*id);
                        }
                    }
                }
            }
            _ => (),
        }

        println!("Updated table ids: {:?}", updated_table_ids);
        for table_id in updated_table_ids {
            self.maybe_raise_blinds(table_id)?;
            self.update_tables(effect, table_id)?;
        }
        self.apply_settles(effect)?;
        effect.settles.clear();
        self.sort_ranks();
        if !no_timeout {
            self.handle_dispatch_timeout(effect)?;
        }
        Ok(())
    }

    fn into_checkpoint(self) -> HandleResult<MttCheckpoint> {
        Ok(MttCheckpoint {})
    }
}

impl Mtt {
    fn create_tables(&mut self) -> Result<(), HandleError> {
        let num_of_players = self.ranks.len();
        let num_of_tables = (self.table_size + num_of_players as u8 - 1) / self.table_size;
        for i in 0..num_of_tables {
            let mut player_map = BTreeMap::<String, Player>::default();
            let mut j = i;
            let table_id = i + 1;
            while let Some(r) = self.ranks.get(j as usize) {
                player_map.insert(
                    r.addr.to_owned(),
                    Player::new(r.addr.to_owned(), r.chips, (j / num_of_tables) as u16, 0),
                );
                self.table_assigns.insert(r.addr.to_owned(), table_id);
                j += num_of_tables;
            }
            let sb = self
                .blind_rules
                .first()
                .ok_or(errors::error_empty_blind_rules())?
                .sb_x as u64
                * self.blind_base;
            let bb = self
                .blind_rules
                .first()
                .ok_or(errors::error_empty_blind_rules())?
                .bb_x as u64
                * self.blind_base;
            let game = Holdem {
                sb,
                bb,
                player_map,
                mode: GameMode::Mtt,
                table_size: self.table_size,
                ..Default::default()
            };
            self.games.insert(table_id, game);
        }

        Ok(())
    }

    fn apply_settles(&mut self, effect: &mut Effect) -> Result<(), HandleError> {
        if !effect.settles.is_empty() {
            println!("Settles: {:?}", effect.settles);
        }
        for s in effect.settles.iter() {
            let rank = self
                .ranks
                .iter_mut()
                .find(|r| r.addr.eq(&s.addr))
                .ok_or(errors::error_player_not_found())?;
            match s.op {
                SettleOp::Add(amount) => {
                    rank.chips += amount;
                }
                SettleOp::Sub(amount) => {
                    rank.chips -= amount;
                    if rank.chips == 0 {
                        rank.status = PlayerRankStatus::Out;
                    }
                }
                SettleOp::Eject => {
                    rank.status = PlayerRankStatus::Out;
                }
                _ => (),
            }
        }
        Ok(())
    }

    fn sort_ranks(&mut self) {
        self.ranks.sort_by(|r1, r2| r2.chips.cmp(&r1.chips));
    }

    // Dispatch the ActionTimeout or WaitingTimeout event which has
    // highest priority based on timestamp.  With Mtt mode, sub games
    // wouldn't dispatch ActionTimeout event.  The current acting
    // player can be found in `acting_player` field.  No op if there's
    // an instant dispatch already.
    fn handle_dispatch_timeout(&mut self, effect: &mut Effect) -> Result<(), HandleError> {
        // Option<&str>, the addr of action timeout or none for waiting timeout
        // u64, the expected timeout timestamp
        let mut first_to_timeout: Option<(Option<&str>, u64)> = None;

        for game in self.games.values() {
            let curr_timestamp = if let Some((_, t)) = first_to_timeout {
                t
            } else {
                u64::MAX
            };

            match (&game.acting_player, game.next_game_start) {
                // action timeout only
                (Some(ActingPlayer { addr, clock, .. }), 0) => {
                    if *clock < curr_timestamp {
                        first_to_timeout = Some((Some(addr), *clock))
                    }
                }
                // both
                (Some(ActingPlayer { addr, clock, .. }), next_game_start) => {
                    if *clock < u64::min(next_game_start, curr_timestamp) {
                        first_to_timeout = Some((Some(addr), *clock))
                    } else if next_game_start < u64::min(*clock, curr_timestamp) {
                        first_to_timeout = Some((None, next_game_start))
                    }
                }
                // waiting timeout only
                (None, next_game_start) if next_game_start > 0 => {
                    first_to_timeout = Some((None, next_game_start))
                }
                _ => (),
            }
        }

        println!("Handle dispatch result: {:?}", first_to_timeout);
        if let Some((addr, timestamp)) = first_to_timeout {
            let timeout = timestamp - effect.timestamp();
            if let Some(addr) = addr {
                println!("Dispatch action timeout");
                effect.action_timeout(addr, timeout)
            } else {
                println!("Dispatch wait timeout");
                effect.wait_timeout(timeout);
            }
        }

        Ok(())
    }

    fn maybe_raise_blinds(&mut self, table_id: TableId) -> Result<(), HandleError> {
        let time_spend = self.time_spend;
        let level = time_spend / self.blind_interval;
        let mut blind_rule = self.blind_rules.get(level as usize);
        if blind_rule.is_none() {
            blind_rule = self.blind_rules.last();
        }
        let blind_rule = blind_rule.ok_or(errors::error_empty_blind_rules())?;
        let sb = blind_rule.sb_x as u64 * self.blind_base;
        let bb = blind_rule.bb_x as u64 * self.blind_base;
        let game = self
            .games
            .get_mut(&table_id)
            .ok_or(errors::error_table_not_fonud())?;
        game.sb = sb;
        game.bb = bb;
        Ok(())
    }

    /// Update tables by balancing the players.
    fn update_tables(&mut self, effect: &mut Effect, table_id: TableId) -> Result<(), HandleError> {
        // No-op for final table
        if self.games.len() == 1 {
            self.table_updates.insert(table_id, TableUpdate::Noop);
            return Ok(());
        }

        let table_size = self.table_size as usize;

        let table_update = {
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

            println!("Table with least: {}", table_with_least.0);
            println!("Table with most: {}", table_with_most.0);
            if table_id == *table_with_least.0
                && table_with_least.1.player_map.len() <= total_empty_seats
            {
                TableUpdate::CloseTable {
                    close_table_id: table_id,
                }
            } else if table_id == *table_with_most.0
                && table_with_most.1.player_map.len() > table_with_least.1.player_map.len() + 1
            {
                TableUpdate::MovePlayer {
                    from_table_id: table_id,
                    to_table_id: *table_with_least.0,
                }
            } else {
                TableUpdate::Noop
            }
        };

        self.table_updates.insert(table_id, table_update.clone());

        match table_update {
            TableUpdate::Noop => (),
            TableUpdate::CloseTable { close_table_id } => {
                // Remove this game
                let mut game_to_close = self
                    .games
                    .remove(&close_table_id)
                    .ok_or(errors::error_table_not_fonud())?;

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
                            println!("Player {} will be moved", player_addr);
                            self.table_assigns.insert(player_addr, *id);
                        } else {
                            break;
                        }
                    }

                    game_ref.internal_add_players(moved_players)?;
                    if game_ref.stage == HoldemStage::Init {
                        game_ref.reset_holdem_state()?;
                        game_ref.reset_player_map_status()?;
                        game_ref.internal_start_game(effect)?;
                    }

                    if game_to_close.player_map.is_empty() {
                        break;
                    }
                }
            }
            TableUpdate::MovePlayer {
                from_table_id,
                to_table_id,
            } => {
                let from_table = self
                    .games
                    .get_mut(&from_table_id)
                    .ok_or(errors::error_table_not_fonud())?;
                let (addr, p) = from_table
                    .player_map
                    .pop_first()
                    .ok_or(errors::error_table_is_empty())?;
                let add_players = vec![InternalPlayerJoin {
                    addr: p.addr,
                    chips: p.chips,
                }];
                let to_table = self
                    .games
                    .get_mut(&to_table_id)
                    .ok_or(errors::error_player_not_found())?;
                to_table.internal_add_players(add_players)?;
                self.table_assigns.insert(addr, to_table_id);
            }
        }
        Ok(())
    }
}
