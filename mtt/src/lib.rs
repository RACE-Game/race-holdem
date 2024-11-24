//! # Holdem MTT
//!
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
//! Game bundle address on Facade:
//! raceholdemtargetraceholdemmtttablewasm

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
    table_id: u8,
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
    alives: usize, // The number of alive players
    stage: MttStage,
    table_assigns: BTreeMap<u64, u8>,
    ranks: Vec<PlayerRank>,
    tables: BTreeMap<u8, MttTableState>,
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
    launched_table_ids: Vec<u8>,
}

impl GameHandler for Mtt {
    fn init_state(init_account: InitAccount) -> HandleResult<Self> {
        let MttAccountData {
            start_time,
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
                        for (id, table) in self.tables.iter() {
                            if !self.launched_table_ids.contains(&id) {
                                self.launch_table(effect, &table)?;
                            }
                        }
                    }
                    _ => {}
                }
            }

            Event::Join { players } => match self.stage {
                MttStage::Init => {
                    for p in players {
                        self.ranks.push(PlayerRank {
                            id: p.id(),
                            chips: self.start_chips,
                            status: PlayerRankStatus::Alive,
                            position: p.position(),
                        });
                        self.total_prize += self.ticket;
                    }
                }
                _ => (),
            },

            Event::Leave { .. } => {
                return Err(error_leave_not_allowed());
            }

            Event::GameStart => {
                self.timestamp = effect.timestamp();
                if self.ranks.is_empty() {
                    // No player joined, mark game as completed
                    self.stage = MttStage::Completed;
                } else if self.ranks.len() == 1 {
                    // Only 1 player joined, end game with single winner
                    self.apply_prizes(effect)?;
                } else {
                    // Start game normally
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
                        effect.checkpoint();
                    }
                    _ => return Err(errors::error_invalid_bridge_event()),
                }
            }

            Event::SubGameReady { game_id } => {
                self.launched_table_ids.push(game_id as u8);
                effect.checkpoint();
            }

            Event::WaitingTimeout => match self.stage {
                // Scheduled game start
                MttStage::Init => {
                    if self.ranks.len() < 2 {
                        self.stage = MttStage::Completed;
                        effect.checkpoint();
                    } else {
                        effect.start_game();
                    }
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

    fn launch_table(&self, effect: &mut Effect, table: &MttTableState) -> Result<(), HandleError> {
        effect.launch_sub_game(
            table.table_id as _,
            self.subgame_bundle.clone(),
            self.table_size as _,
            table,
        )?;
        Ok(())
    }

    fn create_tables(&mut self, effect: &mut Effect) -> Result<(), HandleError> {
        if !self.tables.is_empty() {
            for table in self.tables.values() {
                self.launch_table(effect, table)?;
            }
        } else {
            let num_of_players = self.ranks.len();
            let num_of_tables = (self.table_size + num_of_players as u8 - 1) / self.table_size;
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
                    self.table_assigns.insert(r.id, table_id);
                    j += num_of_tables;
                }
                let (sb, bb) = self.calc_blinds()?;
                let table = MttTableState {
                    table_id,
                    btn: 0,
                    sb,
                    bb,
                    players,
                    next_game_start: 0,
                    hand_id: 0,
                };
                self.launch_table(effect, &table)?;
                self.tables.insert(table_id, table);
            }
        }

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
        self.ranks.sort_by(|r1, r2| {
            r2.chips
                .cmp(&r1.chips)
                .then_with(|| r1.position.cmp(&r2.position))
        });
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
        prefer_table_id: u8,
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
        close_table_id: u8,
    ) -> HandleResult<()> {
        // Remove the table
        let mut table_to_close = self
            .tables
            .remove(&close_table_id)
            .ok_or(errors::error_table_not_fonud())?;

        // All table ids except the one to close
        let target_table_ids: Vec<u8> = {
            let mut table_refs = self.tables.iter().collect::<Vec<(&u8, &MttTableState)>>();
            table_refs.sort_by_key(|(_id, t)| t.players.len());
            table_refs
                .into_iter()
                .map(|(id, _)| *id)
                .filter(|id| id.ne(&close_table_id))
                .collect()
        };

        let mut relocates = BTreeMap::<usize, Vec<MttTablePlayer>>::default();
        for target_table_id in target_table_ids.iter().cycle() {
            let target_table = self.tables.get(target_table_id).ok_or(errors::error_invalid_index_usage())?;

            if table_to_close.players.is_empty() {
                break;
            }

            if target_table.players.len() < self.table_size as _{
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
                        },
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
            effect.bridge_event(
                table_id,
                HoldemBridgeEvent::Relocate {
                    players,
                },
            )?;
        }
        Ok(())
    }

    fn balance_players_between_tables(
        &mut self,
        effect: &mut Effect,
        from_table_id: u8,
        to_table_id: u8,
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
    fn update_tables(&mut self, effect: &mut Effect, table_id: u8) -> Result<(), HandleError> {

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
                effect.settle(id, prize)?;
            }
        }

        self.stage = MttStage::Completed;
        effect.reset();
        Ok(())
    }

    pub fn get_rank(&self, id: u64) -> Option<&PlayerRank> {
        self.ranks.iter().find(|r| r.id == id)
    }
}

#[cfg(test)]
mod tests {

    use race_api::effect::EmitBridgeEvent;

    use super::*;

    // Create Mtt with a number of players. If `with_tables` is
    // true, also create MttTable states.
    pub fn create_mtt_with_players(player_count: usize, table_size: u8, with_tables: bool) -> Mtt {
        let start_chips = 10000;
        let mut mtt = Mtt {
            table_size,
            start_chips,
            ..Default::default()
        };

        let mut rank_id = 1;
        let mut table_id = 0;

        for i in 0..player_count {
            mtt.ranks.push(PlayerRank {
                id: rank_id,
                chips: start_chips,
                status: PlayerRankStatus::Alive,
                position: rank_id as u16 % table_size as u16,
            });

            if with_tables {
                let table_state = if i % (table_size as usize) == 0 {
                    table_id += 1;
                    let table_state = MttTableState {
                        table_id,
                        btn: 0,
                        sb: 100,
                        bb: 200,
                        players: vec![],
                        ..Default::default()
                    };
                    mtt.tables.insert(table_id, table_state);
                    let table_state = mtt.tables.get_mut(&table_id).unwrap();
                    table_state
                } else {
                    mtt.tables.get_mut(&table_id).unwrap()
                };
                let player = MttTablePlayer::new(rank_id, start_chips, 0);
                table_state.players.push(player);
                mtt.table_assigns.insert(rank_id, table_id);
            }

            rank_id += 1;
        }
        mtt
    }

    #[test]
    fn test_create_mtt_with_players() {
        let player_count = 15;
        let table_size = 6;
        let mtt = create_mtt_with_players(player_count, table_size, true);
        assert_eq!(mtt.ranks.len(), player_count);
        assert_eq!(mtt.tables.len(), 3);
        let first_table = mtt.tables.values().next().unwrap();
        let last_table = mtt.tables.values().last().unwrap();
        assert_eq!(first_table.players.len(), table_size as usize);
        assert_eq!(last_table.players.len(), 3);
    }

    #[test]
    fn test_create_mtt_without_tables() {
        let player_count = 10;
        let table_size = 6;
        let mtt = create_mtt_with_players(player_count, table_size, false);
        assert_eq!(mtt.ranks.len(), player_count);
        assert_eq!(mtt.tables.len(), 0);
    }

    #[test]
    fn test_ready_event_in_init_stage() {
        let mut mtt = Mtt::default();
        let mut effect = Effect::default();
        mtt.stage = MttStage::Init;
        mtt.start_time = 100;
        effect.timestamp = 50;

        mtt.handle_event(&mut effect, Event::Ready).unwrap();

        assert!(effect.wait_timeout.is_some());
    }

    #[test]
    fn test_ready_event_in_playing_stage_without_launch() {
        let mut mtt = create_mtt_with_players(15, 6, true);
        let mut effect = Effect::default();
        mtt.stage = MttStage::Playing;
        mtt.timestamp = 0;

        mtt.handle_event(&mut effect, Event::Ready).unwrap();

        assert_eq!(effect.launch_sub_games.len(), 3);
    }

    // ----------------------
    // Update table tests
    // ----------------------

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
    fn test_game_result_from_table_with_1_player() {
        // Create three tables with number of players: 3, 3, 2
        let mut mtt = create_mtt_with_players(8, 3, true);
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
            raw: borsh::to_vec(&game_result).unwrap(),
        };

        mtt.handle_event(&mut effect, game_result_event).unwrap();

        assert_eq!(mtt.get_rank(7).unwrap().chips, 0);
        assert_eq!(mtt.get_rank(8).unwrap().chips, 20000);
        assert_eq!(effect.bridge_events, vec![]);
    }

    #[test]
    fn test_game_result_with_table_with_1_player() {
        // Create three tables with number of players: 3, 3, 1
        let mut mtt = create_mtt_with_players(7, 3, true);
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
            raw: borsh::to_vec(&game_result).unwrap(),
        };

        mtt.handle_event(&mut effect, game_result_event).unwrap();

        assert_eq!(mtt.tables.get(&1).unwrap().players.len(), 3);
        assert_eq!(mtt.tables.get(&2).unwrap().players.len(), 2);
        assert_eq!(mtt.tables.get(&3).unwrap().players.len(), 2);
        assert_eq!(
            effect.bridge_events,
            vec![
                EmitBridgeEvent::try_new(
                    3,
                    HoldemBridgeEvent::Relocate {
                        players: vec![MttTablePlayer::new(4, 10000, 1),],
                    },
                )
                .unwrap(),
                EmitBridgeEvent::try_new(
                    2,
                    HoldemBridgeEvent::StartGame {
                        sb: 50,
                        bb: 100,
                        moved_players: vec![4],
                    },
                )
                .unwrap(),
            ]
        );
    }

    #[test]
    fn test_game_result_with_table_to_close() {
        // Create three tables with number of players: 3, 3, 1
        let mut mtt = create_mtt_with_players(7, 3, true);
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
            raw: borsh::to_vec(&game_result).unwrap(),
        };

        mtt.handle_event(&mut effect, game_result_event).unwrap();

        // Table 1 should be closed, two players should be moved to table 3
        // assert_eq!(mtt.tables.len(), 2);
        assert_eq!(mtt.get_rank(2).unwrap().chips, 0);
        assert_eq!(mtt.get_rank(1).unwrap().chips, 20000);
        assert_eq!(
            effect.bridge_events,
            vec![
                EmitBridgeEvent::try_new(1, HoldemBridgeEvent::CloseTable).unwrap(),
                EmitBridgeEvent::try_new(
                    3,
                    HoldemBridgeEvent::Relocate {
                        players: vec![
                            MttTablePlayer::new(3, 10000, 1),
                            MttTablePlayer::new(0, 20000, 2),
                        ],
                    }
                )
                .unwrap(),
            ]
        );
    }
}
