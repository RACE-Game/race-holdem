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

use borsh::{BorshDeserialize, BorshSerialize};
use race_api::{prelude::*, types::SettleOp};
use race_holdem_mtt_base::{
    HoldemBridgeEvent, InitTableData, MttTable, MttTableCheckpoint, MttTablePlayer,
};
use race_proc_macro::game_handler;
use std::collections::BTreeMap;

const SUBGAME_BUNDLE_ADDR: &str = "raceholdemtargetraceholdemmtttablewasm";

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
    blind_info: BlindInfo,
    prize_rules: Vec<u8>,
    theme: Option<String>, // optional NFT theme
}

#[derive(BorshSerialize, BorshDeserialize)]
pub struct MttCheckpoint {
    start_time: u64,
    ranks: Vec<PlayerRankCheckpoint>,
    tables: BTreeMap<u8, MttTable>,
    time_elapsed: u64,
    timestamp: u64,
    total_prize: u64,
    stage: MttStage,
}

fn find_checkpoint_rank_by_pos(
    ranks: &[PlayerRankCheckpoint],
    id: u64,
) -> Result<&PlayerRankCheckpoint, HandleError> {
    ranks
        .iter()
        .find(|rank| rank.id.eq(&id))
        .ok_or(errors::error_player_id_not_found())
}

#[game_handler]
#[derive(Debug, BorshSerialize, BorshDeserialize, Default)]
pub struct Mtt {
    start_time: u64,
    alives: usize, // The number of alive players
    stage: MttStage,
    table_assigns: BTreeMap<u64, u8>,
    ranks: Vec<PlayerRank>,
    tables: BTreeMap<u8, MttTable>,
    table_size: u8,
    time_elapsed: u64,
    timestamp: u64,
    blind_info: BlindInfo,
    prize_rules: Vec<u8>,
    total_prize: u64,
    ticket: u64,
    theme: Option<String>,
}

impl GameHandler for Mtt {
    fn init_state(effect: &mut Effect, init_account: InitAccount) -> HandleResult<Self> {
        let MttAccountData {
            start_time,
            ticket,
            table_size,
            mut blind_info,
            prize_rules,
            theme,
        } = init_account.data()?;
        let checkpoint: Option<MttCheckpoint> = init_account.checkpoint()?;

        let (start_time, tables, time_elapsed, stage, table_assigns, ranks, total_prize, timestamp) =
            if let Some(checkpoint) = checkpoint {
                let mut table_assigns = BTreeMap::<u64, u8>::new();
                let mut ranks = Vec::<PlayerRank>::new();

                for p in init_account.players.into_iter() {
                    let GamePlayer { id, position, .. } = p;
                    let r = find_checkpoint_rank_by_pos(&checkpoint.ranks, id)?;
                    table_assigns.insert(id, r.table_id);
                    ranks.push(PlayerRank {
                        id,
                        chips: r.chips,
                        status: if r.chips > 0 {
                            PlayerRankStatus::Alive
                        } else {
                            PlayerRankStatus::Out
                        },
                        position,
                    });
                }
                (
                    checkpoint.start_time,
                    checkpoint.tables,
                    checkpoint.time_elapsed,
                    checkpoint.stage,
                    table_assigns,
                    ranks,
                    checkpoint.total_prize,
                    checkpoint.timestamp,
                )
            } else {
                // Initialize an empty game when there's no checkpoint available.
                (
                    start_time,
                    BTreeMap::<u8, MttTable>::new(),
                    0, // no time elapsed
                    MttStage::Init,
                    BTreeMap::<u64, u8>::new(),
                    Vec::<PlayerRank>::new(),
                    0, // no prize at the init
                    0, // no timestamp at the init
                )
            };

        let alives: usize = ranks
            .iter()
            .filter(|rank| rank.status == PlayerRankStatus::Alive)
            .count();

        blind_info.with_default_blind_rules();

        let mut state = Self {
            start_time,
            alives,
            stage,
            table_assigns,
            ranks,
            tables,
            table_size,
            time_elapsed,
            timestamp,
            blind_info,
            prize_rules,
            total_prize,
            ticket,
            theme,
        };

        // Create tables from checkpoint state
        state.create_tables(effect)?;

        effect.allow_exit(false);

        Ok(state)
    }

    fn handle_event(&mut self, effect: &mut Effect, event: Event) -> Result<(), HandleError> {
        // Update time elapsed for blinds calculation.
        self.timestamp = effect.timestamp();
        if self.stage == MttStage::Playing {
            self.time_elapsed = self.time_elapsed + effect.timestamp() - self.timestamp;
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
                            chips: p.balance,
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
                    effect.checkpoint(self.build_checkpoint()?);
                }
            },

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
                effect.checkpoint(self.build_checkpoint()?);
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
                        effect.checkpoint(self.build_checkpoint()?);
                    }
                    _ => return Err(errors::error_invalid_bridge_event()),
                }
            }

            Event::WaitingTimeout => match self.stage {
                MttStage::Init => {
                    if self.ranks.len() < 2 {
                        self.stage = MttStage::Completed;
                        effect.checkpoint(self.build_checkpoint()?);
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
        table: &MttTable,
    ) -> Result<(), HandleError> {
        let mut players = Vec::new();
        for p in table.players.iter() {
            players.push(GamePlayer::new(p.id, p.chips, p.table_position as _));
        }
        let init_table_data = InitTableData {
            table_id,
            table_size: self.table_size,
        };
        effect.launch_sub_game(
            table_id as _,
            SUBGAME_BUNDLE_ADDR.to_string(),
            self.table_size as _,
            players,
            init_table_data,
            Some(MttTableCheckpoint::new(&table)),
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
                let table = MttTable {
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
            let mut table_refs = self.tables.iter().collect::<Vec<(&u8, &MttTable)>>();
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
            // Simply make a checkpoint is the game is on going
            return Ok(());
        }

        let total_shares: u8 = self.prize_rules.iter().take(self.ranks.len()).sum();
        let prize_share: u64 = self.total_prize / total_shares as u64;

        // Get eligible ids for prizes
        for (i, rank) in self.ranks.iter().enumerate() {
            let id = rank.id;
            let rule = self.prize_rules.get(i).unwrap_or(&0);
            let prize: u64 = prize_share * *rule as u64;
            let change: i128 = prize as i128 - self.ticket as i128;
            // Tested safe with 9-zero numbers
            if change > 0 {
                effect.settle(Settle::add(id, change as u64))?;
            } else if change < 0 {
                effect.settle(Settle::sub(id, -change as u64))?;
            } else {
            }
        }

        self.stage = MttStage::Completed;
        effect.allow_exit(true);
        Ok(())
    }

    fn build_checkpoint(&self) -> HandleResult<MttCheckpoint> {
        let Mtt {
            start_time,
            time_elapsed,
            timestamp,
            tables,
            ranks,
            table_assigns,
            total_prize,
            ..
        } = self;

        let mut ranks_checkpoint = Vec::new();

        for rank in ranks {
            let PlayerRank { id, chips, .. } = rank;
            // We use zero `table_id` to indicates there's no table assign for a player
            let table_id = table_assigns.get(&id).cloned().unwrap_or(0);
            ranks_checkpoint.push(PlayerRankCheckpoint {
                id: *id,
                chips: *chips,
                table_id,
            });
        }

        Ok(MttCheckpoint {
            start_time: *start_time,
            timestamp: *timestamp,
            ranks: ranks_checkpoint,
            tables: tables.clone(),
            time_elapsed: *time_elapsed,
            total_prize: *total_prize,
            stage: self.stage,
        })
    }
}

#[cfg(test)]
mod test_t {
    use std::println;

    use super::*;

    #[test]
    fn s() {
        // remote
        let s1 = Mtt::try_from_slice(&[
            244, 138, 72, 8, 145, 1, 0, 0, 4, 0, 0, 0, 0, 0, 0, 0, 1, 4, 0, 0, 0, 2, 0, 0, 0, 0, 0,
            0, 0, 1, 3, 0, 0, 0, 0, 0, 0, 0, 2, 4, 0, 0, 0, 0, 0, 0, 0, 1, 5, 0, 0, 0, 0, 0, 0, 0,
            2, 4, 0, 0, 0, 2, 0, 0, 0, 0, 0, 0, 0, 32, 130, 253, 5, 0, 0, 0, 0, 0, 0, 0, 3, 0, 0,
            0, 0, 0, 0, 0, 32, 130, 253, 5, 0, 0, 0, 0, 0, 1, 0, 4, 0, 0, 0, 0, 0, 0, 0, 224, 63,
            238, 5, 0, 0, 0, 0, 0, 2, 0, 5, 0, 0, 0, 0, 0, 0, 0, 224, 63, 238, 5, 0, 0, 0, 0, 0, 3,
            0, 2, 0, 0, 0, 1, 7, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 32, 161, 7, 0, 0, 0,
            0, 0, 64, 66, 15, 0, 0, 0, 0, 0, 2, 0, 0, 0, 2, 0, 0, 0, 0, 0, 0, 0, 32, 130, 253, 5,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 4, 0, 0, 0, 0, 0, 0, 0, 224, 63, 238, 5, 0, 0, 0,
            0, 1, 0, 0, 0, 0, 0, 0, 0, 141, 196, 74, 8, 145, 1, 0, 0, 2, 7, 0, 0, 0, 0, 0, 0, 0, 1,
            0, 0, 0, 0, 0, 0, 0, 32, 161, 7, 0, 0, 0, 0, 0, 64, 66, 15, 0, 0, 0, 0, 0, 2, 0, 0, 0,
            3, 0, 0, 0, 0, 0, 0, 0, 32, 130, 253, 5, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 5, 0, 0,
            0, 0, 0, 0, 0, 224, 63, 238, 5, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 141, 196, 74, 8,
            145, 1, 0, 0, 3, 0, 0, 0, 0, 0, 0, 0, 0, 6, 177, 74, 8, 145, 1, 0, 0, 32, 161, 7, 0, 0,
            0, 0, 0, 96, 234, 0, 0, 0, 0, 0, 0, 6, 0, 0, 0, 1, 0, 2, 0, 2, 0, 4, 0, 4, 0, 8, 0, 8,
            0, 16, 0, 16, 0, 32, 0, 32, 0, 64, 0, 3, 0, 0, 0, 50, 30, 20, 0, 132, 215, 23, 0, 0, 0,
            0, 0, 225, 245, 5, 0, 0, 0, 0, 0,
        ])
        .unwrap();

        let s2 = Mtt::try_from_slice(&[
            244, 138, 72, 8, 145, 1, 0, 0, 4, 0, 0, 0, 0, 0, 0, 0, 1, 4, 0, 0, 0, 2, 0, 0, 0, 0, 0,
            0, 0, 1, 3, 0, 0, 0, 0, 0, 0, 0, 2, 4, 0, 0, 0, 0, 0, 0, 0, 1, 5, 0, 0, 0, 0, 0, 0, 0,
            2, 4, 0, 0, 0, 2, 0, 0, 0, 0, 0, 0, 0, 32, 130, 253, 5, 0, 0, 0, 0, 0, 0, 0, 3, 0, 0,
            0, 0, 0, 0, 0, 32, 130, 253, 5, 0, 0, 0, 0, 0, 1, 0, 4, 0, 0, 0, 0, 0, 0, 0, 224, 63,
            238, 5, 0, 0, 0, 0, 0, 2, 0, 5, 0, 0, 0, 0, 0, 0, 0, 224, 63, 238, 5, 0, 0, 0, 0, 0, 3,
            0, 2, 0, 0, 0, 1, 7, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 32, 161, 7, 0, 0, 0,
            0, 0, 64, 66, 15, 0, 0, 0, 0, 0, 2, 0, 0, 0, 2, 0, 0, 0, 0, 0, 0, 0, 32, 130, 253, 5,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 4, 0, 0, 0, 0, 0, 0, 0, 224, 63, 238, 5, 0, 0, 0,
            0, 1, 0, 0, 0, 0, 0, 0, 0, 141, 196, 74, 8, 145, 1, 0, 0, 2, 7, 0, 0, 0, 0, 0, 0, 0, 1,
            0, 0, 0, 0, 0, 0, 0, 32, 161, 7, 0, 0, 0, 0, 0, 64, 66, 15, 0, 0, 0, 0, 0, 2, 0, 0, 0,
            3, 0, 0, 0, 0, 0, 0, 0, 32, 130, 253, 5, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 5, 0, 0,
            0, 0, 0, 0, 0, 224, 63, 238, 5, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 141, 196, 74, 8,
            145, 1, 0, 0, 3, 0, 0, 0, 0, 0, 0, 0, 0, 244, 138, 72, 8, 145, 1, 0, 0, 32, 161, 7, 0,
            0, 0, 0, 0, 96, 234, 0, 0, 0, 0, 0, 0, 6, 0, 0, 0, 1, 0, 2, 0, 2, 0, 4, 0, 4, 0, 8, 0,
            8, 0, 16, 0, 16, 0, 32, 0, 32, 0, 64, 0, 3, 0, 0, 0, 50, 30, 20, 0, 132, 215, 23, 0, 0,
            0, 0, 0, 225, 245, 5, 0, 0, 0, 0, 0,
        ])
        .unwrap();

        println!("remote: {:?}", s1);
        println!("local:  {:?}", s2);
    }
}

#[cfg(test)]
mod tests;
