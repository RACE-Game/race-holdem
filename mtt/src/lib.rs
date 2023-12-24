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
use race_holdem_base::{
    essential::{ActingPlayer, GameEvent, GameMode, HoldemStage, InternalPlayerJoin, Player},
    game::Holdem,
};
use race_holdem_mtt_base::{MttTableCheckpoint, MttTablePlayer};
use race_proc_macro::game_handler;
use std::{collections::BTreeMap, string::Drain};

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
    position: u16,
    table_id: u8,
}

#[derive(Debug, BorshSerialize, BorshDeserialize, PartialEq, Eq)]
pub struct PlayerRankCheckpoint {
    mtt_position: u16,
    chips: u64,
    table_id: u8,
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

#[derive(Default, BorshSerialize, BorshDeserialize)]
pub struct BlindInfo {
    blind_base: u64,
    blind_interval: u64,
    blind_rules: Vec<BlindRuleItem>,
}

#[derive(Default, BorshSerialize, BorshDeserialize)]
pub struct MttAccountData {
    start_time: u64,
    table_size: u8,
    blind_info: BlindInfo,
}

#[derive(BorshSerialize, BorshDeserialize)]
pub struct MttCheckpoint {
    start_time: u64,
    ranks: Vec<PlayerRankCheckpoint>,
    tables: BTreeMap<TableId, MttTableCheckpoint>,
    time_elapsed: u64,
}

impl MttCheckpoint {
    fn find_rank_by_pos(&self, mtt_position: u16) -> Result<&PlayerRankCheckpoint, HandleError> {
        self.ranks
            .iter()
            .find(|rank| rank.mtt_position.eq(&mtt_position))
            .ok_or(errors::error_player_mtt_position_not_found())
    }
}

impl From<Mtt> for MttCheckpoint {
    fn from(value: Mtt) -> Self {
        let Mtt {
            start_time,
            blind_info,
            time_elapsed,
            tables,
            ranks,
            ..
        } = value;

        let ranks = ranks
            .into_iter()
            .map(
                |PlayerRank {
                     chips,
                     position,
                     table_id,
                     ..
                 }| PlayerRankCheckpoint {
                    mtt_position: position,
                    chips,
                    table_id,
                },
            )
            .collect::<Vec<PlayerRankCheckpoint>>();

        Self {
            start_time,
            ranks,
            tables,
            time_elapsed,
        }
    }
}

#[game_handler]
#[derive(BorshSerialize, BorshDeserialize, Default)]
pub struct Mtt {
    start_time: u64,
    alives: usize, // The number of alive players
    stage: MttStage,
    table_assigns: BTreeMap<String, TableId>, // The mapping from player address to its table id
    ranks: Vec<PlayerRank>,
    tables: BTreeMap<TableId, MttTableCheckpoint>,
    table_size: u8,
    time_elapsed: u64,
    timestamp: u64,
    blind_info: BlindInfo,
}

impl GameHandler for Mtt {
    type Checkpoint = MttCheckpoint;

    fn init_state(effect: &mut Effect, init_account: InitAccount) -> HandleResult<Self> {
        let MttAccountData {
            start_time,
            table_size,
            blind_info,
        } = init_account.data()?;
        let checkpoint: Option<MttCheckpoint> = init_account.checkpoint()?;

        // let mut table_assigns = BTreeMap::<String, TableId>::new();
        let (start_time, tables, time_elapsed, stage, table_assigns, ranks) =
            if let Some(checkpoint) = checkpoint {
                let mut table_assigns = BTreeMap::<String, TableId>::new();
                let mut ranks = Vec::<PlayerRank>::new();

                for p in init_account.players.into_iter() {
                    let table_id = checkpoint.find_rank_by_pos(p.position)?.table_id;
                    let GamePlayer {
                        addr,
                        position,
                        balance,
                    } = p;

                    table_assigns.insert(p.addr.clone(), table_id);

                    ranks.push(PlayerRank {
                        addr,
                        chips: balance,
                        status: if balance > 0 {
                            PlayerRankStatus::Alive
                        } else {
                            PlayerRankStatus::Out
                        },
                        position,
                        table_id,
                    });
                }
                (
                    checkpoint.start_time,
                    checkpoint.tables,
                    checkpoint.time_elapsed,
                    MttStage::Playing,
                    table_assigns,
                    ranks,
                )
            } else {
                (
                    start_time,
                    BTreeMap::<TableId, MttTableCheckpoint>::new(),
                    0,
                    MttStage::Init,
                    BTreeMap::<String, TableId>::new(),
                    Vec::<PlayerRank>::new(),
                )
            };

        let alives: usize = ranks
            .iter()
            .filter(|rank| rank.status == PlayerRankStatus::Alive)
            .count();

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
        })
    }

    fn handle_event(&mut self, effect: &mut Effect, event: Event) -> Result<(), HandleError> {
        // Update time elapsed for blinds calculation.
        self.time_elapsed = self.time_elapsed + effect.timestamp() - self.timestamp;
        self.timestamp = effect.timestamp();

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

            Event::Sync { new_players, .. } => {
            }

            Event::GameStart { .. } => {
            }

            Event::Bridge { dest, raw } => {

            }

            Event::WaitingTimeout => match self.stage {
                MttStage::Init => {
                }
                MttStage::Playing => {
                }
                _ => (),
            },
            _ => (),
        }

        Ok(())
    }

    fn into_checkpoint(self) -> HandleResult<MttCheckpoint> {
        Ok(self.into())
    }
}

impl Mtt {
    fn create_tables(&mut self) -> Result<(), HandleError> {
        let num_of_players = self.ranks.len();
        let num_of_tables = (self.table_size + num_of_players as u8 - 1) / self.table_size;
        for i in 0..num_of_tables {
            let mut players = Vec::<MttTablePlayer>::new();
            let mut j = i;
            let table_id = i + 1;
            while let Some(r) = self.ranks.get(j as usize) {
                players.push(MttTablePlayer::new(r.position, r.chips, (j / num_of_tables) as usize));
                self.table_assigns.insert(r.addr.to_owned(), table_id);
                j += num_of_tables;
            }
            let sb = self
                .blind_info
                .blind_rules
                .first()
                .ok_or(errors::error_empty_blind_rules())?
                .sb_x as u64
                * self.blind_info.blind_base;
            let bb = self
                .blind_info
                .blind_rules
                .first()
                .ok_or(errors::error_empty_blind_rules())?
                .bb_x as u64
                * self.blind_info.blind_base;
            let table = MttTableCheckpoint {
                btn: 0,
                players,
            };

            self.tables.insert(table_id, table);
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

    fn maybe_raise_blinds(&mut self, table_id: TableId) -> Result<(), HandleError> {
        let time_elapsed = self.time_elapsed;
        let level = time_elapsed / self.blind_interval;
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
