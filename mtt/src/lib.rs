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
use race_holdem_mtt_base::{
    ChipsChange, HoldemBridgeEvent, MttTablePlayer, MttTableSitin, MttTableState, PlayerResult, PlayerResultStatus,
};
use race_proc_macro::game_handler;
use std::collections::{btree_map::Entry, BTreeMap};

const BONUS_DISTRIBUTION_DELAY: u64 = 120_000;

#[derive(BorshSerialize, BorshDeserialize, PartialEq, Eq, Default, Debug, Clone, Copy)]
pub enum MttStage {
    #[default]
    Init,
    Playing,
    DistributingPrize,
    Completed,
}

#[derive(Debug, BorshSerialize, BorshDeserialize, PartialEq, Eq, Default)]
pub enum PlayerRankStatus {
    #[default]
    Pending, // The player has been resitting from one table to another
    Play, // The player is sitting on a table and playing
    Out,  // The player has no chips
}

#[derive(Debug, BorshSerialize, BorshDeserialize, PartialEq, Eq)]
pub struct PlayerRank {
    id: u64,
    chips: u64,
    status: PlayerRankStatus,
    position: u16,
    bounty_reward: u64,
    bounty_transfer: u64,
}

impl PlayerRank {
    pub fn new(id: u64, chips: u64, status: PlayerRankStatus, position: u16) -> Self {
        Self {
            id,
            chips,
            status,
            position,
            bounty_reward: 0,
            bounty_transfer: 0,
        }
    }
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


fn take_amount_by_portion(amount: u64, portion: u16) -> u64 {
    amount * portion as u64 / 1000
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
    min_players: u16,
    rake: u16,             // an integer representing the rake (per thousand)
    bounty: u16,           // an integer representing the bounty (per thousand)
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
    // When all players are prize-eligible.
    in_the_money: bool,
    alives: usize, // The number of alive players
    stage: MttStage,
    table_assigns: BTreeMap<u64, GameId>,
    table_assigns_pending: BTreeMap<u64, GameId>,
    ranks: Vec<PlayerRank>,
    tables: BTreeMap<GameId, MttTableState>,
    table_size: u8,
    time_elapsed: u64,
    timestamp: u64,
    start_chips: u64,
    blind_info: BlindInfo,
    prize_rules: Vec<u8>,
    total_prize: u64,
    total_rake: u64,
    total_bounty: u64,
    ticket: u64,
    theme: Option<String>,
    subgame_bundle: String,
    winners: Vec<MttWinner>,
    launched_table_ids: Vec<GameId>,
    // We use this to save the distributed prizes.
    // For example:
    // If the minimum prize for the top 10 is 100,
    // and 11 players are reduced to 10, each will be guaranteed a prize of 100.
    // The structure is a mapping from player_id to its guranteed prize.
    // The special player_id 0 stands for undistributed.
    player_balances: BTreeMap<u64, u64>,
    // The minimal number of player to start this game.
    // When there are fewer players, we cancel the game.
    min_players: u16,
    rake: u16,
    bounty: u16,
}

impl GameHandler for Mtt {
    fn balances(&self) -> Vec<PlayerBalance> {
        self.player_balances
            .iter()
            .map(|(player_id, balance)| PlayerBalance::new(*player_id, *balance))
            .collect()
    }

    fn init_state(init_account: InitAccount) -> HandleResult<Self> {
        let MttAccountData {
            start_time,
            entry_close_time,
            ticket,
            table_size,
            start_chips,
            mut blind_info,
            prize_rules,
            min_players,
            rake,
            bounty,
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
            min_players,
            rake,
            bounty,
            theme,
            subgame_bundle,
            ..Default::default()
        };

        Ok(state)
    }

    fn handle_event(&mut self, effect: &mut Effect, event: Event) -> Result<(), HandleError> {
        // Update time elapsed for blinds calculation.
        if self.stage == MttStage::Playing {
            self.time_elapsed =
                (self.time_elapsed + effect.timestamp()).saturating_sub(self.timestamp);
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
                        self.ranks.push(
                            PlayerRank::new(p.id(), 0, PlayerRankStatus::Pending, p.position())
                        );
                    }
                }
                MttStage::Playing => {
                    if !self.in_the_money && effect.timestamp() <= self.entry_close_time {
                        for p in players {
                            self.add_player_rank(p);
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

                if self.in_the_money {
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
                    let mut pids_to_sit = Vec::new();
                    for d in deposits {
                        let player_id = d.id();
                        if let Some(rank) = self.ranks.iter_mut().find(|r| r.id == player_id) {
                            if rank.chips == 0 {
                                let amount = d.balance();
                                let amount_to_rake = take_amount_by_portion(amount, self.rake);
                                let amount_to_bounty = take_amount_by_portion(amount, self.bounty);
                                let amount_to_prize = amount - amount_to_rake - amount_to_bounty;
                                let bounty_reward = take_amount_by_portion(amount_to_bounty, 500);
                                let bounty_tranfer = amount_to_bounty - bounty_reward;
                                rank.chips = self.start_chips;
                                rank.status = PlayerRankStatus::Pending;
                                rank.bounty_reward += bounty_reward;
                                rank.bounty_transfer += bounty_tranfer;
                                effect.info(format!("Accept player deposit: {}", d.id()));
                                effect.accept_deposit(&d)?;
                                self.total_prize += amount_to_prize;
                                self.total_rake += amount_to_rake;
                                self.total_bounty += amount_to_bounty;
                                pids_to_sit.push(player_id);

                                self.update_player_balances();
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
                    self.sit_players(effect, pids_to_sit)?;
                    self.sort_ranks();
                    self.update_alives();
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
                        player_results,
                        table,
                        ..
                    } => {
                        for p in table.players.iter() {
                            self.table_assigns_pending.remove(&p.id);
                            self.table_assigns.insert(p.id, table.table_id);
                        }
                        self.handle_bounty(&player_results, effect)?;
                        self.tables.insert(table_id, table);
                        self.apply_chips_change(&player_results)?;
                        self.update_tables(effect, table_id)?;
                        self.apply_prizes(effect)?;
                        self.maybe_set_entry_close(effect);
                        effect.checkpoint();
                    }

                    _ => return Err(errors::error_invalid_bridge_event()),
                }
            }

            Event::SubGameReady {
                game_id, init_data, ..
            } => {
                let table = MttTableState::try_from_slice(&init_data)?;
                self.launched_table_ids.push(game_id);
                self.tables.insert(table.table_id, table);
                effect.checkpoint();
            }

            Event::WaitingTimeout => match self.stage {
                // Scheduled game start
                MttStage::Init => {
                    effect.start_game();
                }

                MttStage::Playing => {}

                MttStage::DistributingPrize => {
                    self.distribute_awards(effect);
                }

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
            .filter(|r| matches!(r.status, PlayerRankStatus::Play | PlayerRankStatus::Pending))
            .count()
            == 1
    }

    fn add_player_rank(&mut self, p: GamePlayer) {
        self.ranks.push(PlayerRank::new(p.id(), 0, PlayerRankStatus::Pending, p.position()));
    }

    fn launch_table(
        &mut self,
        effect: &mut Effect,
        table: MttTableState,
    ) -> Result<(), HandleError> {
        effect.launch_sub_game(self.subgame_bundle.clone(), self.table_size as _, &table)?;
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

        self.maybe_set_in_the_money();

        Ok(())
    }

    fn handle_bounty(&mut self, player_results: &[PlayerResult], effect: &mut Effect) -> Result<(), HandleError> {
        let eliminated_pids: Vec<u64> = player_results.iter().filter(|r| r.status == PlayerResultStatus::Eliminated).map(|r| r.player_id).collect();

        if eliminated_pids.is_empty() {
            // No player is eliminated
            return Ok(())
        }

        let winner_pids: Vec<u64> = player_results.iter().filter(|r| matches!(r.chips_change, Some(ChipsChange::Add(_)))).map(|r| r.player_id).collect();
        let winners_count: u64 = winner_pids.len() as u64;
        let mut total_bounty_reward = 0;
        let mut total_bounty_transfer = 0;

        for pid in eliminated_pids {
            let rank = self.get_rank_mut(pid).ok_or(errors::error_player_not_found())?;
            total_bounty_reward += rank.bounty_reward;
            total_bounty_transfer += rank.bounty_transfer;
            rank.bounty_reward = 0;
            rank.bounty_transfer = 0;
        }

        let bounty_reward = total_bounty_reward / winners_count;
        let bounty_transfer = total_bounty_transfer / winners_count as u64;

        for pid in winner_pids {
            let rank = self.get_rank_mut(pid).ok_or(errors::error_player_not_found())?;
            rank.bounty_reward += bounty_transfer;
            effect.info(format!("Increase bounty of player {} to {}", rank.id, rank.bounty_reward));
            effect.info(format!("Send bounty {} to player {}", bounty_reward, rank.id));
            effect.withdraw(rank.id, bounty_reward);
        }

        // put the rest money into the prize pool
        let remain = total_bounty_reward - (winners_count * bounty_reward) + total_bounty_transfer - (winners_count * bounty_transfer);
        if remain > 0 {
            self.total_prize += remain;
            effect.info(format!("Increase prize to {}", self.total_prize));
        }

        self.total_bounty = self.ranks.iter().map(|r| r.bounty_reward + r.bounty_transfer).sum();

        self.update_player_balances();

        Ok(())
    }

    fn apply_chips_change(
        &mut self,
        player_results: &[PlayerResult],
    ) -> Result<(), HandleError> {
        for rst in player_results {
            let rank = self
                .ranks
                .iter_mut()
                .find(|r| r.id.eq(&rst.player_id))
                .ok_or(errors::error_player_not_found())?;
            match rst.chips_change {
                Some(ChipsChange::Add(amount)) => {
                    rank.chips += amount;
                }
                Some(ChipsChange::Sub(amount)) => {
                    rank.chips -= amount;
                    if rank.chips == 0 {
                        rank.status = PlayerRankStatus::Out;
                        // In such case, we want to unset player's assignment to table
                        self.table_assigns.remove(&rank.id);
                    }
                }
                _ => {}
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
            .filter(|r| r.status != PlayerRankStatus::Out)
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
            let count = self.count_table_players(*id);
            if count < table_with_least_players.2 {
                table_with_least_players = (id, table, count);
            } else if count > table_with_most_players.2 {
                table_with_most_players = (id, table, count);
            }
        }

        Ok((table_with_least_players.1, table_with_most_players.1))
    }

    /// Close the table with `close_table_id`.
    fn close_table_and_move_players_to_other_tables(
        &mut self,
        effect: &mut Effect,
        close_table_id: GameId,
    ) -> HandleResult<()> {
        let Some(table) = self.tables.get(&close_table_id) else {
            return Err(errors::error_table_not_fonud());
        };
        // Remove all table_assigns
        effect.info(format!("Close table {}", close_table_id));
        let player_ids: Vec<u64> = table.players.iter().map(|p| p.id).collect();
        for pid in player_ids.iter() {
            self.table_assigns.remove(pid);
        }
        effect.bridge_event(close_table_id as _, HoldemBridgeEvent::CloseTable)?;
        self.tables.remove(&close_table_id);
        effect.info(format!("Move these player {:?} to other tables", player_ids));
        self.sit_players(effect, player_ids)?;
        Ok(())
    }

    fn balance_players_between_tables(
        &mut self,
        effect: &mut Effect,
        from_table_id: GameId,
        to_table_id: GameId,
        num_to_move: usize,
    ) -> HandleResult<()> {
        let players_to_move: Vec<MttTablePlayer> = self
            .tables
            .get_mut(&from_table_id)
            .ok_or(errors::error_invalid_index_usage())?
            .players
            .drain(0..num_to_move).collect();

        let (sb, bb) = self.calc_blinds()?;
        let sitout_players: Vec<u64> = players_to_move.iter().map(|p| p.id).collect();
        sitout_players.iter().for_each(|pid| {
            self.table_assigns.remove(&pid);
            self.table_assigns_pending.insert(*pid, to_table_id);
        });

        effect.bridge_event(
            from_table_id as _,
            HoldemBridgeEvent::StartGame {
                sb,
                bb,
                sitout_players,
            },
        )?;

        effect.bridge_event(
            to_table_id as _,
            HoldemBridgeEvent::SitinPlayers {
                sitins: players_to_move.iter().map(|p| MttTableSitin::new(p.id, p.chips)).collect(),
            }
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
                effect.info(format!("Send start game to final table {}", table_id));
                let (sb, bb) = self.calc_blinds()?;
                effect.bridge_event(
                    table_id as _,
                    HoldemBridgeEvent::StartGame {
                        sb,
                        bb,
                        sitout_players: Vec::with_capacity(0),
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
                        sitout_players: Vec::with_capacity(0),
                    },
                )?;
            }
        }

        self.maybe_set_in_the_money();

        Ok(())
    }

    fn maybe_set_entry_close(&self, effect: &mut Effect) {
        if effect.timestamp() > self.entry_close_time {
            effect.set_entry_lock(EntryLock::Closed);
        }
    }

    fn maybe_set_in_the_money(&mut self) {
        if self.alives <= self.prize_rules.len() {
            self.in_the_money = true;
        }
    }

    fn distribute_awards(&mut self, effect: &mut Effect) {
        for (i, rank) in self.ranks.iter().enumerate() {
            // We support at most 8 bonuses
            // Identifier = i + 1
            if i > 7 {
                break;
            }
            let id = rank.id;
            effect.award(id, &(i + 1).to_string());
        }
        self.stage = MttStage::Completed;
    }

    fn count_table_players(&self, table_id: GameId) -> usize {
        self.tables.get(&table_id).map(|t| t.players.len()).unwrap_or(0)
            + self.table_assigns_pending.values().filter(|tid| **tid == table_id).count()
    }

    fn find_sparse_non_empty_table(&self) -> Option<GameId> {
        let mut table_id_with_least_players = None;
        let mut least_player_num: usize = self.table_size as _;


        for table_id in self.tables.keys() {
            // The table with least player but not empty
            let count = self.count_table_players(*table_id);
            if count < least_player_num && count > 0 {
                table_id_with_least_players = Some(*table_id);
                least_player_num = count;
            }
        }
        table_id_with_least_players
    }

    /// Add a new player to the game.
    ///
    /// NB: The game will launch tables when receiving StartGame
    /// event, so this function is No-op in Init stage.
    fn sit_players(&mut self, effect: &mut Effect, player_ids: Vec<u64>) -> HandleResult<()> {
        if self.stage == MttStage::Init {
            effect.info("Skip sitting player, because the tourney is not started yet.");
            return Ok(());
        }

        let mut table_id_to_sitins = BTreeMap::<usize, Vec<MttTableSitin>>::new();
        let mut tables_to_launch = Vec::<MttTableState>::new();

        let (sb, bb) = self.calc_blinds()?;

        for player_id in player_ids {

            let last_table_to_launch = tables_to_launch.last_mut();

            let table_to_sit = self.find_sparse_non_empty_table();

            let Some(rank) = self.ranks.iter_mut().find(|r| r.id == player_id) else {
                return Err(errors::error_player_id_not_found())?;
            };

            if let Some(table_id) = table_to_sit {
                // Find a table to sit
                effect.info(format!("Sit player {} to {}", rank.id, table_id));
                let sitin = MttTableSitin::new(rank.id, rank.chips);
                match table_id_to_sitins.entry(table_id) {
                    Entry::Vacant(vacant_entry) => {
                        vacant_entry.insert(vec![sitin]);
                    }
                    Entry::Occupied(mut occupied_entry) => {
                        occupied_entry.get_mut().push(sitin);
                    }
                };
                self.table_assigns_pending.insert(rank.id, table_id);
            } else if last_table_to_launch.as_ref().is_some_and(|t| t.players.len() < self.table_size as _) {
                let Some(table) = last_table_to_launch else {
                    return Err(errors::error_table_not_fonud())?;
                };
                effect.info(format!("Sit player {} to the table just created {}", rank.id, table.table_id));
                let position = table.players.len();
                let player = MttTablePlayer::new(rank.id, rank.chips, position);
                table.players.push(player);
            } else {
                // Table is full, create a new table with this player.
                let table_id = effect.next_sub_game_id();
                effect.info(format!("Create {} table for player {}", table_id, rank.id));
                let player = MttTablePlayer::new(rank.id, rank.chips, 0);
                let players = vec![player];
                let table = MttTableState::new(table_id, sb, bb, players);
                tables_to_launch.push(table);
            }
        }

        for (table_id, sitins) in table_id_to_sitins {
            effect.bridge_event(table_id, HoldemBridgeEvent::SitinPlayers { sitins })?;
        }

        for table in tables_to_launch {
            self.launch_table(effect, table)?;
        }

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

        self.player_balances.insert(0, 0);

        // Get eligible ids for prizes
        for (i, rank) in self.ranks.iter().enumerate() {
            let id = rank.id;
            if let Some(rule) = self.prize_rules.get(i) {
                let prize: u64 = prize_share * *rule as u64;
                self.winners.push(MttWinner {
                    player_id: id,
                    prize,
                });
                effect.withdraw(id, prize + rank.bounty_reward + rank.bounty_transfer);
            }
        }

        self.stage = MttStage::DistributingPrize;
        effect.info("Schedule bonus distribution");
        effect.wait_timeout(BONUS_DISTRIBUTION_DELAY);

        // We also collect the rake when distributing prizes
        effect.transfer(self.total_rake);

        Ok(())
    }

    pub fn get_rank(&self, id: u64) -> Option<&PlayerRank> {
        self.ranks.iter().find(|r| r.id == id)
    }

    pub fn get_rank_mut(&mut self, id: u64) -> Option<&mut PlayerRank> {
        self.ranks.iter_mut().find(|r| r.id == id)
    }

    pub fn update_player_balances(&mut self) {
        let amount = self.total_prize + self.total_rake + self.total_bounty;
        self.player_balances.insert(0, amount);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    mod helper;
    mod misc;
    use helper::*;

    mod test_handle_bounty;
    mod test_handle_game_result;
    mod test_sit_players;
    mod test_start_game;
}
