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
use race_api::{
    prelude::*,
    types::{EntryType, SettleOp},
};
use race_holdem_base::essential::Player;
use race_holdem_mtt_base::{HoldemBridgeEvent, InitTableData, MttTableCheckpoint, MttTablePlayer};
use race_proc_macro::game_handler;
use std::collections::BTreeMap;

const SUBGAME_BUNDLE_ADDR: &str = "raceholdemtargetraceholdemmtttablewasm";

pub type TableId = u8;
pub type PlayerId = u64;

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
    [(1, 2), (2, 3), (3, 6), (8, 12), (12, 16), (16, 20)]
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
    table_size: u8,
    blind_info: BlindInfo,
    prize_rules: Vec<u8>,
    theme: Option<String>, // optional NFT theme
}

#[derive(BorshSerialize, BorshDeserialize)]
pub struct MttCheckpoint {
    start_time: u64,
    ranks: Vec<PlayerRankCheckpoint>,
    tables: BTreeMap<TableId, MttTableCheckpoint>,
    time_elapsed: u64,
    total_prize: u64,
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

impl TryFrom<Mtt> for MttCheckpoint {
    type Error = HandleError;

    fn try_from(value: Mtt) -> HandleResult<Self> {
        let Mtt {
            start_time,
            time_elapsed,
            tables,
            ranks,
            table_assigns,
            total_prize,
            ..
        } = value;

        let mut ranks_checkpoint = Vec::new();

        for rank in ranks {
            let PlayerRank { id, chips, .. } = rank;
            let table_id = *table_assigns
                .get(&id)
                .ok_or(errors::error_player_not_found())?;
            ranks_checkpoint.push(PlayerRankCheckpoint {
                id,
                chips,
                table_id,
            });
        }

        Ok(Self {
            start_time,
            ranks: ranks_checkpoint,
            tables,
            time_elapsed,
            total_prize,
        })
    }
}

#[game_handler]
#[derive(BorshSerialize, BorshDeserialize, Default)]
pub struct Mtt {
    start_time: u64,
    alives: usize, // The number of alive players
    stage: MttStage,
    table_assigns: BTreeMap<PlayerId, TableId>,
    ranks: Vec<PlayerRank>,
    tables: BTreeMap<TableId, MttTableCheckpoint>,
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
    type Checkpoint = MttCheckpoint;

    fn init_state(effect: &mut Effect, init_account: InitAccount) -> HandleResult<Self> {
        let MttAccountData {
            start_time,
            table_size,
            mut blind_info,
            prize_rules,
            theme,
        } = init_account.data()?;
        let checkpoint: Option<MttCheckpoint> = init_account.checkpoint()?;

        let ticket = match init_account.entry_type {
            EntryType::Cash { min_deposit, .. } => min_deposit,
            EntryType::Ticket { amount, .. } => amount,
            EntryType::Gating { .. } => {
                panic!("Entry type not supported");
            }
        };

        let (start_time, tables, time_elapsed, stage, table_assigns, ranks, total_prize) =
            if let Some(checkpoint) = checkpoint {
                let mut table_assigns = BTreeMap::<PlayerId, TableId>::new();
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
                    MttStage::Playing,
                    table_assigns,
                    ranks,
                    checkpoint.total_prize,
                )
            } else {
                (
                    start_time,
                    BTreeMap::<TableId, MttTableCheckpoint>::new(),
                    0, // no time elapsed at the init
                    MttStage::Init,
                    BTreeMap::<u64, TableId>::new(),
                    Vec::<PlayerRank>::new(),
                    0, // no prize at the init
                )
            };

        effect.allow_exit(false);

        let alives: usize = ranks
            .iter()
            .filter(|rank| rank.status == PlayerRankStatus::Alive)
            .count();

        blind_info.with_default_blind_rules();

        Ok(Self {
            start_time,
            alives,
            stage,
            table_assigns,
            ranks,
            tables,
            table_size,
            time_elapsed,
            timestamp: effect.timestamp,
            blind_info,
            prize_rules,
            total_prize,
            ticket,
            theme,
        })
    }

    fn handle_event(&mut self, effect: &mut Effect, event: Event) -> Result<(), HandleError> {
        // Update time elapsed for blinds calculation.
        self.time_elapsed = self.time_elapsed + effect.timestamp() - self.timestamp;
        self.timestamp = effect.timestamp();

        match event {
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
                        effect.settle(Settle::eject(p.id))
                    }
                    effect.checkpoint();
                }
            },

            Event::GameStart { .. } => {
                self.start_time = effect.timestamp();
                self.stage = MttStage::Playing;
                self.create_tables(effect)?;
                self.update_alives();
            }

            Event::Bridge { raw, .. } => {
                let bridge_event = HoldemBridgeEvent::try_parse(&raw)?;
                match bridge_event {
                    HoldemBridgeEvent::GameResult {
                        table_id,
                        settles,
                        checkpoint,
                    } => {
                        self.tables.insert(table_id, checkpoint);
                        self.apply_settles(settles)?;
                        self.update_tables(effect, table_id)?;
                        self.apply_prizes(effect)?;
                    }
                    _ => return Err(errors::error_invalid_bridge_event()),
                }
            }

            Event::WaitingTimeout => match self.stage {
                MttStage::Init => {
                    if self.ranks.len() < 2 {
                        self.stage = MttStage::Completed;
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

    fn into_checkpoint(self) -> HandleResult<MttCheckpoint> {
        Ok(self.try_into()?)
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
        table: &MttTableCheckpoint,
    ) -> Result<(), HandleError> {
        let (sb, bb) = self.calc_blinds()?;
        let mut player_lookup = BTreeMap::new();
        for p in table.players.iter() {
            let player = Player::new(p.id, p.chips, p.table_position as _, 0);
            player_lookup.insert(p.id, player);
        }
        let init_table_data = InitTableData {
            btn: table.btn,
            table_id,
            sb,
            bb,
            table_size: self.table_size,
            player_lookup,
        };
        let checkpoint = MttTableCheckpoint::default();
        effect.launch_sub_game(
            table_id as _,
            SUBGAME_BUNDLE_ADDR.to_string(),
            vec![],
            init_table_data,
            checkpoint,
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
                let table = MttTableCheckpoint { btn: 0, players };
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

    /// Update tables by balancing the players at each table.
    fn update_tables(&mut self, effect: &mut Effect, table_id: TableId) -> Result<(), HandleError> {
        // No-op for final table
        if self.tables.len() == 1 {
            return Ok(());
        }

        let table_size = self.table_size as usize;

        let Some(first_table) = self.tables.first_key_value() else {
            return Ok(())
        };

        let mut table_with_least = first_table;
        let mut table_with_most = first_table;

        for (id, table) in self.tables.iter() {
            if table.players.len() < table_with_least.1.players.len() {
                table_with_least = (id, table);
            }
            if table.players.len() > table_with_most.1.players.len() {
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
            let mut table_refs = self
                .tables
                .iter()
                .collect::<Vec<(&TableId, &MttTableCheckpoint)>>();
            table_refs.sort_by_key(|(_id, t)| t.players.len());
            table_refs
                .into_iter()
                .map(|(id, _)| *id)
                .filter(|id| id.ne(&table_id))
                .collect()
        };

        let mut evts = Vec::<(TableId, HoldemBridgeEvent)>::new();
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
                    evts.push((
                        target_table_ids[i],
                        HoldemBridgeEvent::Relocate {
                            players: vec![player],
                        },
                    ));
                } else {
                    break;
                }
            }

            evts.push((table_id, HoldemBridgeEvent::CloseTable));
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

            players.iter_mut().for_each(|p| table_ref.add_player(p));

            let moved_players = players.iter().map(|p| p.id).collect();

            evts.push((smallest_table_id, HoldemBridgeEvent::Relocate { players }));

            let (sb, bb) = self.calc_blinds()?;
            evts.push((
                table_id,
                HoldemBridgeEvent::StartGame {
                    sb,
                    bb,
                    moved_players,
                },
            ));
        }
        for (table_id, evt) in evts {
            effect.bridge_event(table_id as _, evt)?;
        }

        Ok(())
    }

    fn apply_prizes(&mut self, effect: &mut Effect) -> HandleResult<()> {
        if !self.has_winner() {
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
                effect.settle(Settle::add(id, change as u64));
            } else if change < 0 {
                effect.settle(Settle::sub(id, -change as u64))
            } else {
            }
        }

        self.stage = MttStage::Completed;
        effect.checkpoint();
        effect.allow_exit(true);
        Ok(())
    }
}

#[cfg(test)]
mod tests;
