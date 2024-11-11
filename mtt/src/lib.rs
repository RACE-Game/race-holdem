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
//! Game bundle address on Facade:
//! raceholdemtargetraceholdemmtttablewasm

mod errors;

use borsh::{BorshDeserialize, BorshSerialize};
use errors::error_not_completed;
use race_api::{prelude::*, types::SettleOp};
use race_holdem_mtt_base::{
    HoldemBridgeEvent, InitTableData, MttTableState, MttTablePlayer,
};
use race_proc_macro::game_handler;
use std::collections::BTreeMap;

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
    sb_x: u16,
    bb_x: u16,
}

impl BlindRuleItem {
    fn new(sb_x: u16, bb_x: u16) -> Self {
        Self { sb_x, bb_x }
    }
}

fn default_blind_rules() -> Vec<BlindRuleItem> {
    [(1, 2), (2, 4), (4, 8), (8, 16), (16, 32), (32, 64)]
        .into_iter()
        .map(|(sb, bb)| BlindRuleItem::new(sb, bb))
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
}

impl GameHandler for Mtt {
    fn init_state(effect: &mut Effect, init_account: InitAccount) -> HandleResult<Self> {
        if let Some(checkpoint) = init_account.checkpoint::<Self>()? {
            if !checkpoint.tables.is_empty() {
                for (id, table) in checkpoint.tables.iter() {
                    checkpoint.launch_table(effect, *id, table)?;
                }
            }

            return Ok(checkpoint);
        }

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
                if self.start_time > effect.timestamp {
                    // Schedule the startup
                    effect.wait_timeout(self.start_time - effect.timestamp);
                } else {
                    effect.wait_timeout(0);
                }
            }

            Event::Join { players } => match self.stage {
                MttStage::Init => {
                    for p in players {
                        self.ranks.push(PlayerRank {
                            id: p.id,
                            chips: self.start_chips,
                            status: PlayerRankStatus::Alive,
                            position: p.position,
                        });
                        self.total_prize += self.ticket;
                    }
                }
                _ => {
                    for p in players {
                        effect.settle(Settle::eject(p.id))?;
                    }
                    effect.checkpoint();
                }
            },

            Event::Leave { player_id } => {
                // Only allow leaving when the tourney is completed
                if self.stage.eq(&MttStage::Completed) {
                    // Eject this player to return the prize
                    effect.settle(Settle::eject(player_id))?;
                    effect.checkpoint();
                } else {
                    return Err(error_not_completed());
                }
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
                        settles,
                        table,
                        ..
                    } => {
                        self.tables.insert(table_id, table);
                        self.apply_settles(settles)?;
                        self.update_tables(effect, table_id)?;
                        self.apply_prizes(effect)?;
                        effect.checkpoint();
                    }
                    _ => return Err(errors::error_invalid_bridge_event()),
                }
            }

            Event::WaitingTimeout => match self.stage {
                // Scheduled game start
                MttStage::Init => {
                    if self.ranks.len() < 2 {
                        self.stage = MttStage::Completed;
                        effect.allow_exit(true);
                        effect.checkpoint();
                    } else {
                        effect.start_game();
                    }
                }
                MttStage::Playing => {}
                _ => (),
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

    fn launch_table(
        &self,
        effect: &mut Effect,
        table_id: u8,
        table: &MttTableState,
    ) -> Result<(), HandleError> {
        let mut players = Vec::new();
        let (start_sb, start_bb) = self.start_blinds()?;
        for p in table.players.iter() {
            players.push(GamePlayer::new(p.id, p.chips, p.table_position as _));
        }
        let init_table_data = InitTableData {
            table_id,
            start_sb,
            start_bb,
        };
        effect.launch_sub_game(
            table_id as _,
            self.subgame_bundle.clone(),
            self.table_size as _,
            players,
            init_table_data,
        )?;
        Ok(())
    }

    fn create_tables(&mut self, effect: &mut Effect) -> Result<(), HandleError> {
        if !self.tables.is_empty() {
            for (id, table) in self.tables.iter() {
                self.launch_table(effect, *id, table)?;
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
                    btn: 0,
                    sb,
                    bb,
                    players,
                    next_game_start: 0,
                    hand_id: 0,
                };
                self.launch_table(effect, table_id, &table)?;
                self.tables.insert(table_id, table);
            }
        }

        Ok(())
    }

    fn apply_settles(&mut self, settles: Vec<Settle>) -> Result<(), HandleError> {
        for s in settles.iter() {
            let rank = self
                .ranks
                .iter_mut()
                .find(|r| r.id.eq(&s.id))
                .ok_or(errors::error_player_not_found())?;
            match s.op {
                SettleOp::Add(amount) => {
                    rank.chips += amount;
                }
                SettleOp::Sub(amount) => {
                    rank.chips -= amount;
                    if rank.chips == 0 {
                        rank.status = PlayerRankStatus::Out;
                        // In such case, we want to unset player's assignment to table
                        self.table_assigns.remove(&rank.id);
                    }
                }
                _ => (),
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

    fn start_blinds(&self) -> Result<(u64, u64), HandleError> {
        let blind_rule = self
            .blind_info
            .blind_rules
            .first()
            .ok_or(errors::error_empty_blind_rules())?;
        let sb = blind_rule.sb_x as u64 * self.blind_info.blind_base;
        let bb = blind_rule.bb_x as u64 * self.blind_info.blind_base;
        Ok((sb, bb))
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

    /// Update tables by balancing the players at each table.
    fn update_tables(&mut self, effect: &mut Effect, table_id: u8) -> Result<(), HandleError> {
        let (sb, bb) = self.calc_blinds()?;

        // No-op for final table
        if self.tables.len() == 1 {
            let Some((_, final_table)) = self.tables.first_key_value() else {
                return Err(errors::error_table_not_fonud());
            };
            if final_table.players.len() > 1 {
                effect.bridge_event(
                    table_id as _,
                    HoldemBridgeEvent::StartGame {
                        sb,
                        bb,
                        moved_players: Vec::with_capacity(0),
                    },
                    vec![],
                )?;
            }
            return Ok(());
        }

        let table_size = self.table_size as usize;

        let Some(curr_table) = self.tables.get_key_value(&table_id) else {
            return Err(errors::error_invalid_table_id());
        };

        let mut table_with_least = curr_table;
        let mut table_with_most = curr_table;

        // Find the table with least players and the table with most
        // players, current table has higher priority.
        for (id, table) in self.tables.iter() {
            let l = table.players.len();
            if l < table_with_least.1.players.len() {
                table_with_least = (id, table);
            }
            if l > table_with_most.1.players.len() {
                table_with_most = (id, table);
            }
        }

        let smallest_table_id = *table_with_least.0;
        let largest_table_id = *table_with_most.0;
        let smallest_table_players_count = table_with_least.1.players.len();
        let largest_table_players_count = table_with_most.1.players.len();
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
        let target_table_ids: Vec<u8> = {
            let mut table_refs = self.tables.iter().collect::<Vec<(&u8, &MttTableState)>>();
            table_refs.sort_by_key(|(_id, t)| t.players.len());
            table_refs
                .into_iter()
                .map(|(id, _)| *id)
                .filter(|id| id.ne(&table_id))
                .collect()
        };

        if table_id == smallest_table_id && smallest_table_players_count <= total_empty_seats {
            // Close current table, move players to other tables
            let mut table_to_close = self
                .tables
                .remove(&table_id)
                .ok_or(errors::error_table_not_fonud())?;

            let mut i = 0;
            loop {
                if i == target_table_ids.len() {
                    i = 0;
                    continue;
                }

                let target_table_id = target_table_ids
                    .get(i)
                    .ok_or(errors::error_invalid_index_usage())?;

                if let Some(mut player) = table_to_close.players.pop() {
                    let table_ref = self
                        .tables
                        .get_mut(&target_table_id)
                        .ok_or(errors::error_table_not_fonud())?;
                    table_ref.add_player(&mut player);
                    self.table_assigns
                        .entry(player.id)
                        .and_modify(|v| *v = *target_table_id);
                    effect.bridge_event(
                        target_table_ids[i] as _,
                        HoldemBridgeEvent::Relocate {
                            players: vec![player.clone()],
                        },
                        vec![GamePlayer::new(
                            player.id,
                            player.chips,
                            player.table_position as _,
                        )],
                    )?;
                } else {
                    break;
                }
            }

            effect.bridge_event(table_id as _, HoldemBridgeEvent::CloseTable, vec![])?;
        } else if table_id == largest_table_id
            && largest_table_players_count > smallest_table_players_count + 1
        {
            // Move players to the table with least players
            let num_to_move = (largest_table_players_count - smallest_table_players_count) / 2;
            let mut players: Vec<MttTablePlayer> = self
                .tables
                .get_mut(&largest_table_id)
                .ok_or(errors::error_invalid_index_usage())?
                .players
                .drain(0..num_to_move)
                .collect();

            let table_ref = self
                .tables
                .get_mut(&smallest_table_id)
                .ok_or(errors::error_table_not_fonud())?;

            for p in players.iter_mut() {
                table_ref.add_player(p);
                self.table_assigns
                    .entry(p.id)
                    .and_modify(|v| *v = smallest_table_id);
            }

            let moved_players = players.iter().map(|p| p.id).collect();

            effect.bridge_event(
                smallest_table_id as _,
                HoldemBridgeEvent::Relocate {
                    players: players.clone(),
                },
                players
                    .into_iter()
                    .map(|p| GamePlayer::new(p.id, p.chips, p.table_position as _))
                    .collect(),
            )?;

            effect.bridge_event(
                table_id as _,
                HoldemBridgeEvent::StartGame {
                    sb,
                    bb,
                    moved_players,
                },
                vec![],
            )?;
        } else {
            let Some(table) = self.tables.get(&table_id) else {
                return Err(errors::error_invalid_table_id())
            };

            // Send `StartGame` when there's more than 1 players,
            // Otherwise this table should wait another table for
            // merging.
            if table.players.len() > 1 {
                effect.bridge_event(
                    table_id as _,
                    HoldemBridgeEvent::StartGame {
                        sb,
                        bb,
                        moved_players: Vec::with_capacity(0),
                    },
                    vec![],
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
            let rule = self.prize_rules.get(i).unwrap_or(&0);
            let prize: u64 = prize_share * *rule as u64;
            self.winners.push(MttWinner { player_id: id, prize });
            // Assign the slot to winner
            effect.settle(Settle::assign(id, format!("{}", i + 1)))?;
        }

        self.stage = MttStage::Completed;
        Ok(())
    }
}

#[cfg(test)]
mod test_t {

    use super::*;

    #[test]
    fn test_deser() {
        let s = [72, 197, 88, 238, 146, 1, 0, 0, 0, 225, 245, 5, 0, 0, 0, 0, 3, 16, 39, 0, 0, 0, 0, 0, 0, 50, 0, 0, 0, 0, 0, 0, 0, 96, 234, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 3, 0, 0, 0, 50, 30, 20, 0];
        let account_data: MttAccountData = MttAccountData::try_from_slice(&s).unwrap();
    }
}

// #[cfg(test)]
// mod tests;
