//! # Holdem MTT
//!
//! A Multi-Table Tournament for Hold'em.
//!
//! ## Stages
//!
//! There are three stages in the whole game progress:
//! - Init, the initial state, players can buy-in.
//! - Playing, the game is in progress.
//! - Completed, the game is finished.
//!
//! ## Table Launchment
//!
//! The game starts at timestamp `start-time`, which is saved in the
//! account data.  Depends on the number of registered players and the
//! `table_size`, a list of tables will be created when the game
//! starts.  The same data structure as in cash table is used for each
//! table in the tournament.
//!
//! ## Entry
//!
//! The supported entry type is `Ticket` which supports only one
//! deposit amount.  After a player is eliminated, he can join again
//! by rebuy a ticket with a same amount of deposit.  This must be
//! done before `entry_close_time` or Final Table stage.  An invalid
//! deposit will be rejected immediately.
//!
//! ## Settlement
//!
//! The game ends when only one player remains.  The prizes are
//! distributed based on the proportion define in `prize_rules`(value
//! by per thousand).

mod errors;

use borsh::{BorshDeserialize, BorshSerialize};
use errors::error_leave_not_allowed;
use race_api::prelude::*;
use race_holdem_mtt_base::{ChipsChange, HoldemBridgeEvent, MttTablePlayer, MttTableState};
use race_proc_macro::game_handler;
use std::collections::{btree_map::Entry, BTreeMap};

#[derive(BorshSerialize, BorshDeserialize, PartialEq, Eq, Default, Debug, Clone, Copy)]
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
    id: u64,
    chips: u64,
    status: PlayerRankStatus,
    position: u16,
}

impl PlayerRank {
    pub fn new(id: u64, chips: u64, status: PlayerRankStatus, position: u16) -> Self {
        Self {
            id,
            chips,
            status,
            position,
        }
    }
}

#[derive(Debug, BorshSerialize, BorshDeserialize, PartialEq, Eq)]
pub struct PlayerRankCheckpoint {
    id: u64,
    chips: u64,
    table_id: GameId,
}

#[derive(BorshSerialize, BorshDeserialize, Debug, PartialEq, Eq)]
pub struct BlindRuleItem {
    sb_x: u32,
    bb_x: u32,
}

impl BlindRuleItem {
    fn new(sb_x: u32, bb_x: u32) -> Self {
        Self { sb_x, bb_x }
    }
}

fn default_blind_rules() -> Vec<BlindRuleItem> {
    [
        5, 10, 15, 20, 30, 40, 60, 80, 100, 120, 160, 200, 240, 280, 360, 440, 520, 600, 750, 900,
        1050, 1200, 1500, 1800, 2100, 2400, 3000, 3600, 4200, 5000, 5800, 6600, 7200, 8000, 9000,
        10000, 11000, 12000, 14000, 16000, 18000, 20000, 24000, 28000, 32000, 36000, 40000, 44000,
        50000, 56000, 62000, 68000, 80000, 100000,
    ]
    .into_iter()
    .map(|sb| BlindRuleItem::new(sb, 2 * sb))
    .collect()
}

#[derive(BorshSerialize, BorshDeserialize, Debug, PartialEq, Eq)]
pub struct BlindInfo {
    blind_base: u64,
    blind_interval: u64,
    blind_rules: Vec<BlindRuleItem>,
}

impl Default for BlindInfo {
    fn default() -> Self {
        Self {
            blind_base: 10,
            blind_interval: 60_000,
            blind_rules: default_blind_rules(),
        }
    }
}

impl BlindInfo {
    pub fn with_default_blind_rules(&mut self) {
        if self.blind_rules.is_empty() {
            self.blind_rules = default_blind_rules();
        }
    }
}

#[derive(Default, BorshSerialize, BorshDeserialize)]
pub struct MttAccountData {
    start_time: u64,
    entry_close_time: u64,
    ticket: u64,
    table_size: u8,
    start_chips: u64,
    blind_info: BlindInfo,
    prize_rules: Vec<u8>,
    theme: Option<String>, // optional NFT theme
    subgame_bundle: String,
}

#[derive(Debug, BorshSerialize, BorshDeserialize, Default)]
pub struct MttWinner {
    player_id: u64,
    prize: u64,
}

#[game_handler]
#[derive(Debug, BorshSerialize, BorshDeserialize, Default)]
pub struct Mtt {
    start_time: u64,
    entry_close_time: u64,
    is_final_table: bool,
    alives: usize, // The number of alive players
    stage: MttStage,
    table_assigns: BTreeMap<u64, GameId>,
    ranks: Vec<PlayerRank>,
    tables: BTreeMap<GameId, MttTableState>,
    table_size: u8,
    time_elapsed: u64,
    timestamp: u64,
    start_chips: u64,
    blind_info: BlindInfo,
    prize_rules: Vec<u8>,
    total_prize: u64,
    ticket: u64,
    theme: Option<String>,
    subgame_bundle: String,
    winners: Vec<MttWinner>,
    launched_table_ids: Vec<GameId>,
}

impl GameHandler for Mtt {
    fn init_state(init_account: InitAccount) -> HandleResult<Self> {
        let MttAccountData {
            start_time,
            entry_close_time,
            ticket,
            table_size,
            start_chips,
            mut blind_info,
            prize_rules,
            theme,
            subgame_bundle,
        } = init_account.data()?;

        blind_info.with_default_blind_rules();

        let state = Self {
            start_time,
            entry_close_time,
            table_size,
            start_chips,
            blind_info,
            prize_rules,
            ticket,
            theme,
            subgame_bundle,
            ..Default::default()
        };

        Ok(state)
    }

    fn handle_event(&mut self, effect: &mut Effect, event: Event) -> Result<(), HandleError> {
        // Update time elapsed for blinds calculation.
        if self.stage == MttStage::Playing {
            self.time_elapsed = self.time_elapsed + effect.timestamp() - self.timestamp;
            self.timestamp = effect.timestamp();
        }

        match event {
            Event::Custom { .. } => {
                return Err(errors::error_custom_event_not_allowed())?;
            }

            Event::Ready => {
                match self.stage {
                    // Schedule game start or start directly
                    MttStage::Init => {
                        if self.start_time > effect.timestamp {
                            effect.wait_timeout(self.start_time - effect.timestamp);
                        } else {
                            effect.start_game();
                        }
                    }
                    MttStage::Playing => {
                        // Why we have to launch table here
                        // for (id, table) in self.tables.iter() {
                        //     if !self.launched_table_ids.contains(&id) {
                        //         self.launch_table(effect, &mut table)?;
                        //     }
                        // }
                    }
                    _ => {}
                }
            }

            Event::Join { players } => match self.stage {
                MttStage::Init => {
                    for p in players {
                        self.ranks.push(PlayerRank {
                            id: p.id(),
                            chips: 0,
                            status: PlayerRankStatus::Out,
                            position: p.position(),
                        });
                    }
                }
                MttStage::Playing => {
                    if !self.is_final_table && effect.timestamp() <= self.entry_close_time {
                        for p in players {
                            self.ranks.push(PlayerRank {
                                id: p.id(),
                                chips: 0,
                                status: PlayerRankStatus::Out,
                                position: p.position(),
                            });
                        }
                    } else {
                        for p in players {
                            effect.warn(format!("Reject player join: {}", p.id()));
                        }
                    }
                }
                _ => {
                    effect.warn("Join event at completed stage.");
                    for p in players {
                        effect.warn(format!("Reject player join: {}", p.id()));
                    }
                }
            },

            Event::Deposit { deposits } => {
                // For any case that the player is not in the game,
                // the deposit should be rejected.

                if self.is_final_table {
                    for d in deposits {
                        effect.warn(format!(
                            "Reject player deposit: {} (Final Table Stage)",
                            d.id()
                        ));
                        effect.reject_deposit(&d)?;
                    }
                } else if effect.timestamp() > self.entry_close_time {
                    for d in deposits {
                        effect.warn(format!("Reject player deposit: {} (Entry Close)", d.id()));
                        effect.reject_deposit(&d)?;
                    }
                } else {
                    for d in deposits {
                        let player_id = d.id();
                        if let Some(rank) = self.ranks.iter_mut().find(|r| r.id == player_id) {
                            if rank.chips == 0 {
                                rank.chips = self.start_chips;
                                rank.status = PlayerRankStatus::Alive;
                                effect.info(format!("Accept player deposit: {}", d.id()));
                                effect.accept_deposit(&d)?;
                                self.total_prize += d.balance();
                                self.add_new_player(effect, player_id)?;
                                self.update_alives();
                            } else {
                                effect.warn(format!(
                                    "Reject player deposit: {} (Player Has Chips)",
                                    d.id()
                                ));
                                effect.reject_deposit(&d)?;
                            }
                        } else {
                            effect.warn(format!(
                                "Reject player deposit: {} (Player Not In Game)",
                                d.id()
                            ));
                            effect.reject_deposit(&d)?;
                        }
                    }
                }
            }

            Event::Leave { .. } => {
                return Err(error_leave_not_allowed());
            }

            Event::GameStart => {
                self.timestamp = effect.timestamp();
                if self.ranks.is_empty() {
                    effect.info("Game is empty, just complete the game");
                    // No player joined, mark game as completed
                    self.stage = MttStage::Completed;
                } else if self.ranks.len() == 1 {
                    // Only 1 player joined, end game with single winner
                    effect
                        .info("Game has only one player, set it the winner and complete the game");
                    self.apply_prizes(effect)?;
                } else {
                    // Start game normally
                    effect.info(format!("Start game with {} players", self.ranks.len()));
                    self.stage = MttStage::Playing;
                    self.create_tables(effect)?;
                    self.update_alives();
                }
                effect.checkpoint();
            }

            Event::Bridge { raw, .. } => {
                let bridge_event = HoldemBridgeEvent::try_parse(&raw)?;
                match bridge_event {
                    HoldemBridgeEvent::GameResult {
                        table_id,
                        chips_change,
                        table,
                        ..
                    } => {
                        self.tables.insert(table_id, table);
                        self.apply_chips_change(chips_change)?;
                        self.update_tables(effect, table_id)?;
                        self.apply_prizes(effect)?;
                        self.maybe_set_entry_close(effect);
                        effect.checkpoint();
                    }
                    _ => return Err(errors::error_invalid_bridge_event()),
                }
            }

            Event::SubGameReady { game_id } => {
                self.launched_table_ids.push(game_id);
                effect.checkpoint();
            }

            Event::WaitingTimeout => match self.stage {
                // Scheduled game start
                MttStage::Init => {
                    effect.start_game();
                }

                MttStage::Playing => {}

                MttStage::Completed => {}
            },
            _ => (),
        }

        Ok(())
    }
}

impl Mtt {
    #[allow(dead_code)]
    fn find_player_rank_by_id(&self, id: u64) -> Result<&PlayerRank, HandleError> {
        self.ranks
            .iter()
            .find(|rank| rank.id.eq(&id))
            .ok_or(errors::error_player_id_not_found())
    }

    // When there is only one player alive, the game is over and the one is winner
    fn has_winner(&self) -> bool {
        self.ranks
            .iter()
            .filter(|r| r.status == PlayerRankStatus::Alive)
            .count()
            == 1
    }

    fn launch_table(&mut self, effect: &mut Effect, table: MttTableState) -> Result<(), HandleError> {
        effect.launch_sub_game(self.subgame_bundle.clone(), self.table_size as _, &table)?;
        self.tables.insert(table.table_id, table);
        Ok(())
    }

    fn create_tables(&mut self, effect: &mut Effect) -> Result<(), HandleError> {
        let num_of_players = self.ranks.len();
        let num_of_tables =
            (self.table_size as usize + num_of_players - 1) / self.table_size as usize;
        for i in 0..num_of_tables {
            let mut players = Vec::<MttTablePlayer>::new();
            let mut j = i;
            let table_id = i + 1;
            while let Some(r) = self.ranks.get(j as usize) {
                players.push(MttTablePlayer::new(
                    r.id,
                    r.chips,
                    (j / num_of_tables) as usize, // player's table position
                ));
                self.table_assigns.insert(r.id, table_id as _);
                j += num_of_tables;
            }
            let (sb, bb) = self.calc_blinds()?;
            let table_id = effect.next_sub_game_id();
            let table = MttTableState {
                table_id: table_id.into(),
                btn: 0,
                sb,
                bb,
                players,
                next_game_start: 0,
                hand_id: 0,
            };
            self.launch_table(effect, table)?;
        }

        self.maybe_set_final_table();

        Ok(())
    }

    fn apply_chips_change(
        &mut self,
        chips_change: BTreeMap<u64, ChipsChange>,
    ) -> Result<(), HandleError> {
        for (pid, change) in chips_change.into_iter() {
            let rank = self
                .ranks
                .iter_mut()
                .find(|r| r.id.eq(&pid))
                .ok_or(errors::error_player_not_found())?;
            match change {
                ChipsChange::Add(amount) => {
                    rank.chips += amount;
                }
                ChipsChange::Sub(amount) => {
                    rank.chips -= amount;
                    if rank.chips == 0 {
                        rank.status = PlayerRankStatus::Out;
                        // In such case, we want to unset player's assignment to table
                        self.table_assigns.remove(&rank.id);
                    }
                }
            }
        }
        self.sort_ranks();
        self.update_alives();
        Ok(())
    }

    fn update_alives(&mut self) {
        self.alives = self
            .ranks
            .iter()
            .filter(|r| r.status == PlayerRankStatus::Alive)
            .count();
    }

    fn sort_ranks(&mut self) {
        self.ranks.sort_by(|r1, r2| r2.chips.cmp(&r1.chips));
    }

    fn calc_blinds(&self) -> Result<(u64, u64), HandleError> {
        let time_elapsed = self.time_elapsed;
        let level = time_elapsed / self.blind_info.blind_interval;
        let mut blind_rule = self.blind_info.blind_rules.get(level as usize);
        if blind_rule.is_none() {
            blind_rule = self.blind_info.blind_rules.last();
        }
        let blind_rule = blind_rule.ok_or(errors::error_empty_blind_rules())?;
        let sb = blind_rule.sb_x as u64 * self.blind_info.blind_base;
        let bb = blind_rule.bb_x as u64 * self.blind_info.blind_base;
        Ok((sb, bb))
    }

    /// Return the tables with least players and most players in a
    /// tuple.  The table with `prefer_table_id` will be picked with
    /// higher priority when candidates have the same number of players.
    fn get_table_ids_with_least_most_players(
        &self,
        prefer_table_id: GameId,
    ) -> HandleResult<(&MttTableState, &MttTableState)> {
        let Some(curr_table) = self.tables.get_key_value(&prefer_table_id) else {
            return Err(errors::error_invalid_table_id());
        };

        let default = (curr_table.0, curr_table.1, curr_table.1.players.len());
        let mut table_with_least_players = default;
        let mut table_with_most_players = default;

        for (id, table) in self.tables.iter() {
            let count = table.players.len();
            if count < table_with_least_players.2 {
                table_with_least_players = (id, table, count);
            } else if count > table_with_most_players.2 {
                table_with_most_players = (id, table, count);
            }
        }

        Ok((table_with_least_players.1, table_with_most_players.1))
    }

    /// Close the table with `close_table_id` and move all its players
    /// to other tables.  Remove the closed table from state.
    fn close_table_and_move_players_to_other_tables(
        &mut self,
        effect: &mut Effect,
        close_table_id: GameId,
    ) -> HandleResult<()> {
        // Remove the table
        let mut table_to_close = self
            .tables
            .remove(&close_table_id)
            .ok_or(errors::error_table_not_fonud())?;

        // All table ids except the one to close
        let target_table_ids: Vec<GameId> = {
            let mut table_refs = self
                .tables
                .iter()
                .collect::<Vec<(&GameId, &MttTableState)>>();
            table_refs.sort_by_key(|(_id, t)| t.players.len());
            table_refs
                .into_iter()
                .map(|(id, _)| *id)
                .filter(|id| id.ne(&close_table_id))
                .collect()
        };

        let mut relocates = BTreeMap::<usize, Vec<MttTablePlayer>>::default();
        for target_table_id in target_table_ids.iter().cycle() {
            let target_table = self
                .tables
                .get(target_table_id)
                .ok_or(errors::error_invalid_index_usage())?;

            if table_to_close.players.is_empty() {
                break;
            }

            if target_table.players.len() < self.table_size as _ {
                if let Some(mut player) = table_to_close.players.pop() {
                    let table_ref = self
                        .tables
                        .get_mut(&target_table_id)
                        .ok_or(errors::error_table_not_fonud())?;
                    table_ref.add_player(&mut player);
                    self.table_assigns
                        .entry(player.id)
                        .and_modify(|v| *v = *target_table_id);

                    match relocates.entry(*target_table_id as usize) {
                        Entry::Vacant(e) => {
                            e.insert(vec![player.clone()]);
                        }
                        Entry::Occupied(mut e) => {
                            e.get_mut().push(player.clone());
                        }
                    };
                } else {
                    break;
                }
            }
        }

        effect.bridge_event(close_table_id as _, HoldemBridgeEvent::CloseTable)?;

        for (table_id, players) in relocates {
            effect.bridge_event(table_id as _, HoldemBridgeEvent::Relocate { players })?;
        }
        Ok(())
    }

    fn balance_players_between_tables(
        &mut self,
        effect: &mut Effect,
        from_table_id: GameId,
        to_table_id: GameId,
        num_to_move: usize,
    ) -> HandleResult<()> {
        let mut players: Vec<MttTablePlayer> = self
            .tables
            .get_mut(&from_table_id)
            .ok_or(errors::error_invalid_index_usage())?
            .players
            .drain(0..num_to_move)
            .collect();

        let table_ref = self
            .tables
            .get_mut(&to_table_id)
            .ok_or(errors::error_table_not_fonud())?;

        for p in players.iter_mut() {
            table_ref.add_player(p);
            self.table_assigns
                .entry(p.id)
                .and_modify(|v| *v = to_table_id);
        }

        let moved_players = players.iter().map(|p| p.id).collect();

        effect.bridge_event(
            to_table_id as _,
            HoldemBridgeEvent::Relocate {
                players: players.clone(),
            },
        )?;
        let (sb, bb) = self.calc_blinds()?;
        effect.bridge_event(
            from_table_id as _,
            HoldemBridgeEvent::StartGame {
                sb,
                bb,
                moved_players,
            },
        )?;

        Ok(())
    }

    /// Update tables by balancing the players at each table.
    /// `table_id` is the id of current table, where the bridge event
    /// came from.
    ///
    /// The rules for table balancing:
    ///
    /// When there are enough empty seats to accept all the players
    /// from current table, close current table and move players to
    /// empty seats. In this scenario, a CloseTable event and a few
    /// Relocate events will be emitted.
    ///
    /// When current table is the one with most players and have at
    /// least two more players than the table with least payers, move
    /// one player to that table. In this scenario, a StartGame event
    /// and a Relocate event will be emitted.
    ///
    /// Do nothing when there's only one table.
    fn update_tables(&mut self, effect: &mut Effect, table_id: GameId) -> Result<(), HandleError> {
        // No-op for final table
        if self.tables.len() == 1 {
            let Some((_, final_table)) = self.tables.first_key_value() else {
                return Err(errors::error_table_not_fonud());
            };
            if final_table.players.len() > 1 {
                let (sb, bb) = self.calc_blinds()?;
                effect.bridge_event(
                    table_id as _,
                    HoldemBridgeEvent::StartGame {
                        sb,
                        bb,
                        moved_players: Vec::with_capacity(0),
                    },
                )?;
            }
            return Ok(());
        }

        let Some(current_table) = self.tables.get(&table_id) else {
            Err(errors::error_invalid_table_id())?
        };

        let table_size = self.table_size as usize;

        let (smallest_table, largest_table) =
            self.get_table_ids_with_least_most_players(table_id)?;

        let smallest_table_id = smallest_table.table_id;
        let largest_table_id = largest_table.table_id;
        let smallest_table_players_count = smallest_table.players.len();
        let largest_table_players_count = largest_table.players.len();
        let current_table_players_count = current_table.players.len();
        let total_empty_seats = self
            .tables
            .iter()
            .map(|(id, t)| {
                if *id == table_id {
                    0
                } else {
                    table_size - t.players.len()
                }
            })
            .sum::<usize>();

        if current_table_players_count <= total_empty_seats {
            self.close_table_and_move_players_to_other_tables(effect, table_id)?;
        } else if table_id == largest_table_id
            && largest_table_players_count > smallest_table_players_count + 1
        {
            self.balance_players_between_tables(
                effect,
                largest_table_id,
                smallest_table_id,
                (largest_table_players_count - smallest_table_players_count) / 2,
            )?;
        } else {
            let Some(table) = self.tables.get(&table_id) else {
                return Err(errors::error_invalid_table_id());
            };

            // Send `StartGame` when there's more than 1 players,
            // Otherwise this table should wait another table for
            // merging.
            if table.players.len() > 1 {
                let (sb, bb) = self.calc_blinds()?;
                effect.bridge_event(
                    table_id as _,
                    HoldemBridgeEvent::StartGame {
                        sb,
                        bb,
                        moved_players: Vec::with_capacity(0),
                    },
                )?;
            }
        }

        self.maybe_set_final_table();

        Ok(())
    }

    fn maybe_set_entry_close(&self, effect: &mut Effect) {
        effect.set_entry_lock(EntryLock::Closed);
    }

    fn maybe_set_final_table(&mut self) {
        if self.tables.len() == 1 {
            self.is_final_table = true;
        }
    }

    /// Add a new player to the game.
    ///
    /// NB: The game will launch tables when receiving GameStart
    /// event, so this function is No-op in Init stage.
    fn add_new_player(&mut self, effect: &mut Effect, player_id: u64) -> HandleResult<()> {
        if self.stage == MttStage::Init {
            return Ok(());
        }

        let (sb, bb) = self.calc_blinds()?;

        let Some(rank) = self.ranks.iter_mut().find(|r| r.id == player_id) else {
            return Err(errors::error_player_id_not_found())?;
        };

        let mut player = MttTablePlayer::new(rank.id, rank.chips, 0);

        for (table_id, table) in self.tables.iter_mut() {
            if table.players.len() < self.table_size as usize {
                self.table_assigns.insert(player_id, *table_id);
                table.add_player(&mut player);

                effect.bridge_event(
                    *table_id,
                    HoldemBridgeEvent::Relocate {
                        players: vec![player.clone()],
                    },
                )?;
                effect.info(format!("Add player {} to table {}", player_id, table_id));
                return Ok(());
            }
        }

        // Table is full, create a new table with this player.
        effect.info("Create a table for new player");

        let players = vec![player];

        let table_id = effect.next_sub_game_id();
        let table = MttTableState {
            table_id,
            btn: 0,
            sb,
            bb,
            players,
            next_game_start: 0,
            hand_id: 0,
        };

        self.table_assigns.insert(player_id, table_id);
        self.launch_table(effect, table)?;
        Ok(())
    }

    /// Apply the prizes and mark the game as completed.
    fn apply_prizes(&mut self, effect: &mut Effect) -> HandleResult<()> {
        if !self.has_winner() {
            // Do nothing is the game is ongoing
            return Ok(());
        }

        let total_shares: u8 = self.prize_rules.iter().take(self.ranks.len()).sum();
        let prize_share: u64 = self.total_prize / total_shares as u64;

        // Get eligible ids for prizes
        for (i, rank) in self.ranks.iter().enumerate() {
            let id = rank.id;
            if let Some(rule) = self.prize_rules.get(i) {
                let prize: u64 = prize_share * *rule as u64;
                self.winners.push(MttWinner {
                    player_id: id,
                    prize,
                });
                effect.settle(id, prize, false)?;
            }
        }

        self.stage = MttStage::Completed;
        Ok(())
    }

    pub fn get_rank(&self, id: u64) -> Option<&PlayerRank> {
        self.ranks.iter().find(|r| r.id == id)
    }
}

#[cfg(test)]
mod tests {

    use super::*;

    // Create Mtt with a number of players. If `with_tables` is
    // true, also create MttTable states.
    pub fn create_mtt_with_players(player_nums_per_table: &[usize], table_size: u8) -> Mtt {
        let start_chips = 10000;
        let mut mtt = Mtt {
            table_size,
            start_chips,
            ..Default::default()
        };

        let mut rank_id = 1;
        let mut table_id = 0;

        for player_num in player_nums_per_table {
            table_id += 1;
            let table_state = MttTableState {
                table_id,
                btn: 0,
                sb: DEFAULT_SB,
                bb: DEFAULT_BB,
                players: vec![],
                ..Default::default()
            };

            mtt.tables.insert(table_id, table_state);
            let table_state = mtt.tables.get_mut(&table_id).unwrap();

            for i in 0..*player_num {
                mtt.ranks.push(PlayerRank {
                    id: rank_id,
                    chips: start_chips,
                    status: PlayerRankStatus::Alive,
                    position: rank_id as u16 % table_size as u16,
                });

                let player = MttTablePlayer::new(rank_id, start_chips, i);
                table_state.players.push(player);
                mtt.table_assigns.insert(rank_id, table_id);
                rank_id += 1;
            }
        }

        mtt
    }

    #[test]
    fn create_mtt_with_15_players_in_3_tables() {
        let player_count = 15;
        let table_size = 6;
        let mtt = create_mtt_with_players(&[6, 6, 3], table_size);
        assert_eq!(mtt.ranks.len(), player_count);
        assert_eq!(mtt.tables.len(), 3);
        let first_table = mtt.tables.values().next().unwrap();
        let last_table = mtt.tables.values().last().unwrap();
        assert_eq!(first_table.players.len(), table_size as usize);
        assert_eq!(last_table.players.len(), 3);
    }

    #[test]
    fn test_ready_event_in_init_stage_dispatch_wait_timeout() {
        let mut mtt = Mtt::default();
        let mut effect = Effect::default();
        mtt.stage = MttStage::Init;
        mtt.start_time = 100;
        effect.timestamp = 50;

        mtt.handle_event(&mut effect, Event::Ready).unwrap();

        assert!(effect.wait_timeout.is_some());
    }

    #[test]
    fn test_ready_event_in_playing_stage_launch_sub_games() {
        let mut mtt = create_mtt_with_players(&[6, 6, 2], 6);
        let mut effect = Effect::default();
        mtt.stage = MttStage::Playing;
        mtt.timestamp = 0;

        mtt.handle_event(&mut effect, Event::Ready).unwrap();

        assert_eq!(effect.launch_sub_games.len(), 3);
    }

    // ----------------------
    // Update table tests
    // ----------------------

    const DEFAULT_SB: u64 = 50;
    const DEFAULT_BB: u64 = 100;

    #[test]
    fn test_get_tables_with_least_most_players() {
        let mtt = Mtt {
            tables: BTreeMap::from([
                (
                    1,
                    MttTableState {
                        table_id: 1,
                        players: vec![
                            MttTablePlayer::new(1, 10000, 0),
                            MttTablePlayer::new(2, 10000, 1),
                            MttTablePlayer::new(3, 10000, 2),
                        ],
                        ..Default::default()
                    },
                ),
                (
                    2,
                    MttTableState {
                        table_id: 2,
                        players: vec![
                            MttTablePlayer::new(1, 10000, 0),
                            MttTablePlayer::new(2, 10000, 1),
                        ],
                        ..Default::default()
                    },
                ),
                (
                    3,
                    MttTableState {
                        table_id: 3,
                        players: vec![
                            MttTablePlayer::new(1, 10000, 0),
                            MttTablePlayer::new(1, 10000, 0),
                            MttTablePlayer::new(1, 10000, 0),
                            MttTablePlayer::new(2, 10000, 1),
                        ],
                        ..Default::default()
                    },
                ),
                (
                    4,
                    MttTableState {
                        table_id: 4,
                        players: vec![
                            MttTablePlayer::new(1, 10000, 0),
                            MttTablePlayer::new(2, 10000, 1),
                        ],
                        ..Default::default()
                    },
                ),
                (
                    5,
                    MttTableState {
                        table_id: 5,
                        players: vec![
                            MttTablePlayer::new(1, 10000, 0),
                            MttTablePlayer::new(1, 10000, 0),
                            MttTablePlayer::new(1, 10000, 0),
                            MttTablePlayer::new(2, 10000, 1),
                        ],
                        ..Default::default()
                    },
                ),
            ]),
            ..Default::default()
        };

        let (least, most) = mtt.get_table_ids_with_least_most_players(1).unwrap();
        assert_eq!(least.table_id, 2);
        assert_eq!(most.table_id, 3);

        let (least, most) = mtt.get_table_ids_with_least_most_players(2).unwrap();
        assert_eq!(least.table_id, 2);
        assert_eq!(most.table_id, 3);

        let (least, most) = mtt.get_table_ids_with_least_most_players(3).unwrap();
        assert_eq!(least.table_id, 2);
        assert_eq!(most.table_id, 3);

        let (least, most) = mtt.get_table_ids_with_least_most_players(4).unwrap();
        assert_eq!(least.table_id, 4);
        assert_eq!(most.table_id, 3);

        let (least, most) = mtt.get_table_ids_with_least_most_players(5).unwrap();
        assert_eq!(least.table_id, 2);
        assert_eq!(most.table_id, 5);
    }

    #[test]
    fn test_game_result_given_2_tables_and_current_table_has_1_player_do_dispatch_nothing() {
        // Create three tables with number of players: 3, 3, 2
        let mut mtt = create_mtt_with_players(&[3, 3, 2], 3);
        let mut effect = Effect::default();

        // Player 7 lost to player 8, thus there's only one player on the table
        let game_result = HoldemBridgeEvent::GameResult {
            hand_id: 1,
            table_id: 3,
            chips_change: BTreeMap::from([
                (7, ChipsChange::Sub(10000)),
                (8, ChipsChange::Add(10000)),
            ]),
            table: MttTableState {
                hand_id: 1,
                table_id: 3,
                players: vec![MttTablePlayer::new(8, 20000, 1)],
                next_game_start: 0,
                ..Default::default()
            },
        };
        let game_result_event = Event::Bridge {
            dest_game_id: 0, // the table with two players
            from_game_id: 3,
            raw: borsh::to_vec(&game_result).unwrap(),
        };

        mtt.handle_event(&mut effect, game_result_event).unwrap();

        assert_eq!(mtt.get_rank(7).unwrap().chips, 0);
        assert_eq!(mtt.get_rank(8).unwrap().chips, 20000);
        assert_eq!(effect.bridge_events, vec![]);
    }

    #[test]
    fn test_game_result_given_3_tables_and_current_table_has_1_player_do_dispatch_nothing() {
        // Create three tables with number of players: 3, 3, 2
        let mut mtt = create_mtt_with_players(&[3, 2], 3);
        let mut effect = Effect::default();

        // Player 7 lost to player 8, thus there's only one player on the table
        let game_result = HoldemBridgeEvent::GameResult {
            hand_id: 1,
            table_id: 2,
            chips_change: BTreeMap::from([
                (4, ChipsChange::Sub(10000)),
                (5, ChipsChange::Add(10000)),
            ]),
            table: MttTableState {
                hand_id: 1,
                table_id: 2,
                players: vec![MttTablePlayer::new(8, 20000, 1)],
                next_game_start: 0,
                ..Default::default()
            },
        };
        let game_result_event = Event::Bridge {
            dest_game_id: 0, // the table with two players
            from_game_id: 2,
            raw: borsh::to_vec(&game_result).unwrap(),
        };

        mtt.handle_event(&mut effect, game_result_event).unwrap();

        assert_eq!(mtt.get_rank(4).unwrap().chips, 0);
        assert_eq!(mtt.get_rank(5).unwrap().chips, 20000);
        assert_eq!(effect.bridge_events, vec![]);
    }

    #[test]
    fn test_game_result_given_3_tables_and_single_player_in_different_table_do_dispatch_relocate_and_start_game(
    ) {
        // Create three tables with number of players: 3, 3, 1
        let mut mtt = create_mtt_with_players(&[3, 3, 1], 3);
        let mut effect = Effect::default();

        // Whatever happened on current table should trigger the balancing
        let game_result = HoldemBridgeEvent::GameResult {
            hand_id: 1,
            table_id: 2,
            chips_change: BTreeMap::default(),
            table: MttTableState {
                hand_id: 1,
                table_id: 2,
                players: vec![
                    MttTablePlayer::new(4, 10000, 0),
                    MttTablePlayer::new(5, 10000, 1),
                    MttTablePlayer::new(6, 10000, 2),
                ],
                ..Default::default()
            },
        };

        let game_result_event = Event::Bridge {
            dest_game_id: 0,
            from_game_id: 2,
            raw: borsh::to_vec(&game_result).unwrap(),
        };

        mtt.handle_event(&mut effect, game_result_event).unwrap();

        assert_eq!(mtt.tables.get(&1).unwrap().players.len(), 3);
        assert_eq!(mtt.tables.get(&2).unwrap().players.len(), 2);
        assert_eq!(mtt.tables.get(&3).unwrap().players.len(), 2);
        assert_eq!(
            effect.list_bridge_events().unwrap(),
            vec![
                (
                    3,
                    HoldemBridgeEvent::Relocate {
                        players: vec![MttTablePlayer::new(4, 10000, 1),],
                    },
                ),
                (
                    2,
                    HoldemBridgeEvent::StartGame {
                        sb: DEFAULT_SB,
                        bb: DEFAULT_BB,
                        moved_players: vec![4],
                    },
                )
            ]
        );
    }

    #[test]
    fn test_game_result_given_3_tables_do_close_table() {
        // Create three tables with number of players: 3, 3, 1
        let mut mtt = create_mtt_with_players(&[3, 3, 1], 3);
        let mut effect = Effect::default();

        // Eliminate one player on table 1
        let game_result = HoldemBridgeEvent::GameResult {
            hand_id: 1,
            table_id: 1,
            chips_change: BTreeMap::from([
                (1, ChipsChange::Add(10000)),
                (2, ChipsChange::Sub(10000)),
            ]),
            table: MttTableState {
                hand_id: 1,
                table_id: 1,
                players: vec![
                    MttTablePlayer::new(0, 20000, 0),
                    MttTablePlayer::new(3, 10000, 2),
                ],
                ..Default::default()
            },
        };

        let game_result_event = Event::Bridge {
            dest_game_id: 0,
            from_game_id: 1,
            raw: borsh::to_vec(&game_result).unwrap(),
        };

        mtt.handle_event(&mut effect, game_result_event).unwrap();

        // Table 1 should be closed, two players should be moved to table 3
        // assert_eq!(mtt.tables.len(), 2);
        assert_eq!(mtt.get_rank(2).unwrap().chips, 0);
        assert_eq!(mtt.get_rank(1).unwrap().chips, 20000);
        assert_eq!(
            effect.list_bridge_events().unwrap(),
            vec![
                (1, HoldemBridgeEvent::CloseTable),
                (
                    3,
                    HoldemBridgeEvent::Relocate {
                        players: vec![
                            MttTablePlayer::new(3, 10000, 1),
                            MttTablePlayer::new(0, 20000, 2),
                        ],
                    }
                )
            ]
        );
    }

    #[test]
    fn test_game_result_given_2_tables_do_close_table() {
        // Create three tables with number of players: 3, 3, 1
        let mut mtt = create_mtt_with_players(&[2, 2], 3);
        let mut effect = Effect::default();

        // Eliminate one player on table 1
        let game_result = HoldemBridgeEvent::GameResult {
            hand_id: 1,
            table_id: 1,
            chips_change: BTreeMap::from([
                (1, ChipsChange::Add(10000)),
                (2, ChipsChange::Sub(10000)),
            ]),
            table: MttTableState {
                hand_id: 1,
                table_id: 1,
                players: vec![MttTablePlayer::new(1, 20000, 0)],
                ..Default::default()
            },
        };

        let game_result_event = Event::Bridge {
            dest_game_id: 0,
            from_game_id: 1,
            raw: borsh::to_vec(&game_result).unwrap(),
        };

        mtt.handle_event(&mut effect, game_result_event).unwrap();

        // Table 1 should be closed, two players should be moved to table 3
        // assert_eq!(mtt.tables.len(), 2);
        assert_eq!(mtt.get_rank(2).unwrap().chips, 0);
        assert_eq!(mtt.get_rank(1).unwrap().chips, 20000);
        assert_eq!(
            effect.list_bridge_events().unwrap(),
            vec![
                (1, HoldemBridgeEvent::CloseTable),
                (
                    2,
                    HoldemBridgeEvent::Relocate {
                        players: vec![MttTablePlayer::new(1, 20000, 2),],
                    }
                )
            ]
        );
    }

    #[test]
    fn test_game_result_given_2_tables_move_one_player() {
        // Create three tables with number of players: 3, 3, 1
        let mut mtt = create_mtt_with_players(&[3, 1], 3);
        let mut effect = Effect::default();

        let game_result = HoldemBridgeEvent::GameResult {
            hand_id: 1,
            table_id: 1,
            chips_change: BTreeMap::default(),
            table: MttTableState {
                hand_id: 1,
                table_id: 1,
                players: vec![
                    MttTablePlayer::new(1, 10000, 0),
                    MttTablePlayer::new(2, 10000, 1),
                    MttTablePlayer::new(3, 10000, 2),
                ],
                ..Default::default()
            },
        };

        let game_result_event = Event::Bridge {
            dest_game_id: 0,
            from_game_id: 1,
            raw: borsh::to_vec(&game_result).unwrap(),
        };

        mtt.handle_event(&mut effect, game_result_event).unwrap();

        assert_eq!(
            effect.list_bridge_events().unwrap(),
            vec![
                (
                    2,
                    HoldemBridgeEvent::Relocate {
                        players: vec![MttTablePlayer::new(1, 10000, 1)],
                    }
                ),
                (
                    1,
                    HoldemBridgeEvent::StartGame {
                        moved_players: vec![1],
                        sb: DEFAULT_SB,
                        bb: DEFAULT_BB,
                    }
                )
            ]
        );

        assert_eq!(mtt.table_assigns.get(&1), Some(&2));
        assert_eq!(mtt.table_assigns.get(&2), Some(&1));
        assert_eq!(mtt.table_assigns.get(&3), Some(&1));
        assert_eq!(mtt.table_assigns.get(&4), Some(&2));
        assert_eq!(mtt.tables.get(&1).map(|t| t.players.len()), Some(2));
        assert_eq!(mtt.tables.get(&2).map(|t| t.players.len()), Some(2));
    }

    // Test sort ranks

    #[test]
    fn test_sort_ranks_ensures_eliminated_players_keep_order() {
        let mut mtt = Mtt::default();

        // Active players
        mtt.ranks
            .push(PlayerRank::new(1, 5000, PlayerRankStatus::Alive, 0));
        mtt.ranks
            .push(PlayerRank::new(2, 10000, PlayerRankStatus::Alive, 1));
        mtt.ranks
            .push(PlayerRank::new(3, 7000, PlayerRankStatus::Alive, 2));

        // Eliminated players
        mtt.ranks
            .push(PlayerRank::new(4, 0, PlayerRankStatus::Out, 3));
        mtt.ranks
            .push(PlayerRank::new(5, 0, PlayerRankStatus::Out, 4));

        mtt.sort_ranks();

        assert_eq!(mtt.ranks[0].id, 2);
        assert_eq!(mtt.ranks[1].id, 3);
        assert_eq!(mtt.ranks[2].id, 1);
        assert_eq!(mtt.ranks[3].id, 4);
        assert_eq!(mtt.ranks[4].id, 5);
    }
}
