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
use race_holdem_base::essential::Player;
use race_holdem_mtt_base::{HoldemBridgeEvent, InitTableData, MttTableCheckpoint, MttTablePlayer};
use race_proc_macro::game_handler;
use std::collections::BTreeMap;

const SUBGAME_BUNDLE_ADDR: &str = "";

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
}

#[derive(BorshSerialize, BorshDeserialize)]
pub struct MttCheckpoint {
    start_time: u64,
    ranks: Vec<PlayerRankCheckpoint>,
    tables: BTreeMap<TableId, MttTableCheckpoint>,
    time_elapsed: u64,
}

#[allow(dead_code)]
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
            ..
        } = value;

        let mut ranks_checkpoint = Vec::new();

        for rank in ranks {
            let PlayerRank {id, chips, ..} = rank;
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
        })
    }
}

#[game_handler]
#[derive(BorshSerialize, BorshDeserialize, Default)]
pub struct Mtt {
    start_time: u64,
    alives: usize, // The number of alive players
    stage: MttStage,
    table_assigns: BTreeMap<PlayerId, TableId>, // The mapping from player id to their table id
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
            mut blind_info,
        } = init_account.data()?;
        let checkpoint: Option<MttCheckpoint> = init_account.checkpoint()?;

        let (start_time, tables, time_elapsed, stage, table_assigns, ranks) =
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
                )
            } else {
                (
                    start_time,
                    BTreeMap::<TableId, MttTableCheckpoint>::new(),
                    0,
                    MttStage::Init,
                    BTreeMap::<u64, TableId>::new(),
                    Vec::<PlayerRank>::new(),
                )
            };

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
        })
    }

    fn handle_event(&mut self, effect: &mut Effect, event: Event) -> Result<(), HandleError> {
        // Update time elapsed for blinds calculation.
        self.time_elapsed = self.time_elapsed + effect.timestamp() - self.timestamp;
        self.timestamp = effect.timestamp();

        match event {
            Event::Ready => {
                // Schedule the startup
                if self.start_time > effect.timestamp {
                    effect.wait_timeout(self.start_time - effect.timestamp);
                } else {
                    effect.wait_timeout(0);
                }
            }

            Event::Sync { new_players } => match self.stage {
                MttStage::Init => {
                    for p in new_players {
                        self.ranks.push(PlayerRank {
                            id: p.id,
                            chips: p.balance,
                            status: PlayerRankStatus::Alive,
                            position: p.position,
                        });
                    }
                }
                _ => {
                    for p in new_players {
                        effect.settle(Settle::eject(p.id))
                    }
                    effect.checkpoint();
                }
            },

            Event::GameStart { .. } => {
                self.create_tables(effect)?;
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
                    }
                    _ => return Err(errors::error_invalid_bridge_event()),
                }
            }

            Event::WaitingTimeout => match self.stage {
                MttStage::Init => {}
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
        effect.launch_sub_game(
            table_id as _,
            SUBGAME_BUNDLE_ADDR.to_string(),
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
        self.alives = self
            .ranks
            .iter()
            .filter(|r| r.status == PlayerRankStatus::Alive)
            .count();
        self.sort_ranks();
        Ok(())
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

                if let Some(player) = table_to_close.players.pop() {
                    let table_ref = self
                        .tables
                        .get_mut(&target_table_id)
                        .ok_or(errors::error_table_not_fonud())?;
                    let _pos = table_ref.add_player(player.id, player.chips);
                    evts.push((
                        target_table_ids[i],
                        HoldemBridgeEvent::Relocate { players: vec![player] },
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
            let players: Vec<MttTablePlayer> = self
                .tables
                .get_mut(&largest_table_id)
                .ok_or(errors::error_invalid_index_usage())?
                .players
                .drain(0..num_to_move)
                .collect();
            let moved_players = players.iter().map(|p| p.id).collect();

            evts.push((
                smallest_table_id,
                HoldemBridgeEvent::Relocate { players },
            ));

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
}

#[cfg(test)]
mod tests {
    use super::*;
    use race_test::prelude::*;

    use crate::{MttAccountData, MttCheckpoint};
    fn init_test_state(players: [&mut TestClient; 4]) -> anyhow::Result<Mtt> {
        let mut players_iter = players.into_iter();
        let alice = players_iter.next().unwrap();
        let bob = players_iter.next().unwrap();
        let carol = players_iter.next().unwrap();
        let dave = players_iter.next().unwrap();
        let acc = TestGameAccountBuilder::default()
            .with_data(MttAccountData {
                start_time: 1000,
                table_size: 2,
                blind_info: BlindInfo::default(),
            })
            .add_player(alice, 1000)
            .add_player(bob, 1000)
            .add_player(carol, 2000)
            .add_player(dave, 0)
            .with_checkpoint(MttCheckpoint {
                start_time: 1001,
                ranks: vec![
                    PlayerRankCheckpoint {
                        id: alice.id(),
                        chips: 1000,
                        table_id: 1,
                    },
                    PlayerRankCheckpoint {
                        id: bob.id(),
                        chips: 1000,
                        table_id: 2,
                    },
                    PlayerRankCheckpoint {
                        id: carol.id(),
                        chips: 2000,
                        table_id: 1,
                    },
                    PlayerRankCheckpoint {
                        id: dave.id(),
                        chips: 0,
                        table_id: 2,
                    },
                ],
                tables: BTreeMap::from([
                    (
                        1,
                        MttTableCheckpoint {
                            btn: 0,
                            players: vec![
                                MttTablePlayer {
                                    id: alice.id(),
                                    chips: 1000,
                                    table_position: 0,
                                },
                                MttTablePlayer {
                                    id: carol.id(),
                                    chips: 2000,
                                    table_position: 1,
                                },
                            ],
                        },
                    ),
                    (
                        2,
                        MttTableCheckpoint {
                            btn: 0,
                            players: vec![MttTablePlayer {
                                id: bob.id(),
                                chips: 1000,
                                table_position: 0,
                            }],
                        },
                    ),
                ]),
                time_elapsed: 50,
            })
            .build();
        let init_account = acc.derive_init_account();
        let mut effect = Effect::default();
        Ok(Mtt::init_state(&mut effect, init_account)?)
    }

    // Like init_test_state but with larger table size and more players
    fn setup_mtt_state(players: [&mut TestClient; 12]) -> anyhow::Result<Mtt> {
        let mut players_iter = players.into_iter();
        let pa = players_iter.next().unwrap();
        let pb = players_iter.next().unwrap();
        let pc = players_iter.next().unwrap();
        let pd = players_iter.next().unwrap();
        let pe = players_iter.next().unwrap();
        let pf = players_iter.next().unwrap();
        let pg = players_iter.next().unwrap();
        let ph = players_iter.next().unwrap();
        let pi = players_iter.next().unwrap();
        let pj = players_iter.next().unwrap();
        let pk = players_iter.next().unwrap();
        let pl = players_iter.next().unwrap();

        let acc = TestGameAccountBuilder::default()
            .with_max_players(20)
            .with_data(MttAccountData {
                start_time: 1000,
                table_size: 3,
                blind_info: BlindInfo::default(),
            })
            .add_player(pa, 1000)
            .add_player(pb, 1000)
            .add_player(pc, 1000)
            .add_player(pd, 1000)
            .add_player(pe, 1000)
            .add_player(pf, 1000)
            .add_player(pg, 1000)
            .add_player(ph, 1000)
            .add_player(pi, 1000)
            .add_player(pj, 1000)
            .add_player(pk, 1000)
            .add_player(pl, 1000)
            .with_checkpoint(MttCheckpoint {
                start_time: 1001,
                ranks: vec![
                    PlayerRankCheckpoint {
                        // pa
                        id: pa.id(),
                        chips: 1000,
                        table_id: 1,
                    },
                    PlayerRankCheckpoint {
                        // pb
                        id: pb.id(),
                        chips: 1000,
                        table_id: 2,
                    },
                    PlayerRankCheckpoint {
                        // pc
                        id: pc.id(),
                        chips: 1000,
                        table_id: 3,
                    },
                    PlayerRankCheckpoint {
                        // pd
                        id: pd.id(),
                        chips: 1000,
                        table_id: 4,
                    },
                    PlayerRankCheckpoint {
                        // pe
                        id: pe.id(),
                        chips: 1000,
                        table_id: 1,
                    },
                    PlayerRankCheckpoint {
                        // pf
                        id: pf.id(),
                        chips: 1000,
                        table_id: 2,
                    },
                    PlayerRankCheckpoint {
                        // pg
                        id: pg.id(),
                        chips: 1000,
                        table_id: 3,
                    },
                    PlayerRankCheckpoint {
                        // ph
                        id: ph.id(),
                        chips: 1000,
                        table_id: 4,
                    },
                    PlayerRankCheckpoint {
                        // pi
                        id: pi.id(),
                        chips: 1000,
                        table_id: 1,
                    },
                    PlayerRankCheckpoint {
                        // pj
                        id: pj.id(),
                        chips: 1000,
                        table_id: 2,
                    },
                    PlayerRankCheckpoint {
                        // pk
                        id: pk.id(),
                        chips: 1000,
                        table_id: 3,
                    },
                    PlayerRankCheckpoint {
                        // pl
                        id: pl.id(),
                        chips: 1000,
                        table_id: 4,
                    },
                ],
                tables: BTreeMap::from([
                    (
                        1,
                        MttTableCheckpoint {
                            btn: 0,
                            players: vec![
                                MttTablePlayer {
                                    id: pa.id(),
                                    chips: 1000,
                                    table_position: 0,
                                },
                                MttTablePlayer {
                                    id: pe.id(),
                                    chips: 1000,
                                    table_position: 1,
                                },
                                MttTablePlayer {
                                    id: pi.id(),
                                    chips: 1000,
                                    table_position: 2,
                                },
                            ],
                        },
                    ),
                    (
                        2,
                        MttTableCheckpoint {
                            btn: 0,
                            players: vec![
                                MttTablePlayer {
                                    id: pb.id(),
                                    chips: 1000,
                                    table_position: 0,
                                },
                                MttTablePlayer {
                                    id: pf.id(),
                                    chips: 1000,
                                    table_position: 1,
                                },
                                MttTablePlayer {
                                    id: pj.id(),
                                    chips: 1000,
                                    table_position: 2,
                                },
                            ],
                        },
                    ),
                    (
                        3,
                        MttTableCheckpoint {
                            btn: 0,
                            players: vec![
                                MttTablePlayer {
                                    id: pc.id(),
                                    chips: 1000,
                                    table_position: 0,
                                },
                                MttTablePlayer {
                                    id: pg.id(),
                                    chips: 1000,
                                    table_position: 1,
                                },
                                MttTablePlayer {
                                    id: pk.id(),
                                    chips: 1000,
                                    table_position: 2,
                                },
                            ],
                        },
                    ),
                    (
                        4,
                        MttTableCheckpoint {
                            btn: 0,
                            players: vec![
                                MttTablePlayer {
                                    id: pd.id(),
                                    chips: 1000,
                                    table_position: 0,
                                },
                                MttTablePlayer {
                                    id: ph.id(),
                                    chips: 1000,
                                    table_position: 1,
                                },
                                MttTablePlayer {
                                    id: pl.id(),
                                    chips: 1000,
                                    table_position: 2,
                                },
                            ],
                        },
                    ),
                ]),
                time_elapsed: 50,
            })
            .build();
        let init_account = acc.derive_init_account();
        let mut effect = Effect::default();
        Ok(Mtt::init_state(&mut effect, init_account)?)
    }

    #[test]
    fn test_init_state_with_new_game() -> anyhow::Result<()> {
        let acc = TestGameAccountBuilder::default()
            .with_data(MttAccountData {
                start_time: 1000,
                table_size: 6,
                blind_info: BlindInfo::default(),
            })
            .build();
        let init_account = acc.derive_init_account();
        let mut effect = Effect::default();
        let mtt = Mtt::init_state(&mut effect, init_account)?;
        assert_eq!(mtt.start_time, 1000);
        assert_eq!(mtt.alives, 0);
        assert_eq!(mtt.stage, MttStage::Init);
        assert!(mtt.table_assigns.is_empty());
        assert!(mtt.ranks.is_empty());
        assert!(mtt.tables.is_empty());
        assert_eq!(mtt.table_size, 6);
        assert_eq!(mtt.time_elapsed, 0);
        assert_eq!(mtt.timestamp, effect.timestamp);
        assert_eq!(mtt.blind_info, BlindInfo::default());
        Ok(())
    }

    #[test]
    fn test_init_state_with_checkpoint() -> anyhow::Result<()> {
        let mut alice = TestClient::player("alice");
        let mut bob = TestClient::player("bob");
        let mut carol = TestClient::player("carol");
        let mut dave = TestClient::player("dave");
        let players = [&mut alice, &mut bob, &mut carol, &mut dave];
        let mtt = init_test_state(players)?;
        assert_eq!(mtt.start_time, 1001);
        assert_eq!(mtt.alives, 3);
        assert_eq!(mtt.stage, MttStage::Playing);
        assert_eq!(mtt.table_assigns.len(), 4);
        assert_eq!(mtt.ranks.len(), 4);
        assert_eq!(mtt.tables.len(), 2);
        assert_eq!(mtt.table_size, 2);
        assert_eq!(mtt.time_elapsed, 50);
        assert_eq!(mtt.timestamp, Effect::default().timestamp);
        assert_eq!(mtt.blind_info, BlindInfo::default());
        Ok(())
    }

    #[test]
    fn test_create_tables() -> anyhow::Result<()> {
        let mut alice = TestClient::player("alice");
        let mut bob = TestClient::player("bob");
        let mut carol = TestClient::player("carol");
        let mut dave = TestClient::player("dave");
        let players = [&mut alice, &mut bob, &mut carol, &mut dave];
        let mut mtt = init_test_state(players)?;
        let evt = Event::GameStart { access_version: 0 };
        let mut effect = Effect::default();
        mtt.handle_event(&mut effect, evt)?;
        assert_eq!(
            mtt.table_assigns,
            BTreeMap::from([
                (alice.id(), 1),
                (bob.id(), 2),
                (carol.id(), 1),
                (dave.id(), 2),
            ])
        );
        println!("table_assigns {:?}", mtt.table_assigns);
        assert_eq!(
            effect.launch_sub_games,
            vec![
                LaunchSubGame::try_new(
                    1,
                    SUBGAME_BUNDLE_ADDR.into(),
                    InitTableData {
                        btn: 0,
                        table_id: 1,
                        sb: 10,
                        bb: 20,
                        table_size: 2,
                        player_lookup: BTreeMap::from([
                            (alice.id(), Player::new(alice.id(), 1000, 0, 0)),
                            (carol.id(), Player::new(carol.id(), 2000, 1, 0))
                        ])
                    }
                )?,
                LaunchSubGame::try_new(
                    2,
                    SUBGAME_BUNDLE_ADDR.into(),
                    InitTableData {
                        btn: 0,
                        table_id: 2,
                        sb: 10,
                        bb: 20,
                        table_size: 2,
                        player_lookup: BTreeMap::from(
                            [(bob.id(), Player::new(bob.id(), 1000, 0, 0)),])
                    }
                )?,
            ]
        );

        Ok(())
    }

    #[test]
    fn test_move_players() -> anyhow::Result<()> {
        let mut pa = TestClient::player("pa");
        let mut pb = TestClient::player("pb");
        let mut pc = TestClient::player("pc");
        let mut pd = TestClient::player("pd");
        let mut pe = TestClient::player("pe");
        let mut pf = TestClient::player("pf");
        let mut pg = TestClient::player("pg");
        let mut ph = TestClient::player("ph");
        let mut pi = TestClient::player("pi");
        let mut pj = TestClient::player("pj");
        let mut pk = TestClient::player("pk");
        let mut pl = TestClient::player("pl");
        let players = [
            &mut pa, &mut pb, &mut pc, &mut pd, &mut pe, &mut pf, &mut pg, &mut ph, &mut pi,
            &mut pj, &mut pk, &mut pl,
        ];
        let mut mtt = setup_mtt_state(players)?;
        let evt = Event::GameStart { access_version: 0 };
        let mut effect = Effect::default();
        mtt.handle_event(&mut effect, evt)?;

        assert_eq!(
            mtt.table_assigns,
            BTreeMap::from([
                (pa.id(), 1),
                (pb.id(), 2),
                (pc.id(), 3),
                (pd.id(), 4),
                (pe.id(), 1),
                (pf.id(), 2),
                (pg.id(), 3),
                (ph.id(), 4),
                (pi.id(), 1),
                (pj.id(), 2),
                (pk.id(), 3),
                (pl.id(), 4),
            ])
        );

        assert_eq!(
            effect.launch_sub_games,
            vec![
                LaunchSubGame::try_new(
                    1,
                    SUBGAME_BUNDLE_ADDR.into(),
                    InitTableData {
                        btn: 0,
                        table_id: 1,
                        sb: 10,
                        bb: 20,
                        table_size: 3,
                        player_lookup: BTreeMap::from([
                            (pa.id(), Player::new(pa.id(), 1000, 0, 0)),
                            (pe.id(), Player::new(pe.id(), 1000, 1, 0)),
                            (pi.id(), Player::new(pi.id(), 1000, 2, 0)),
                        ])
                    }
                )?,
                LaunchSubGame::try_new(
                    2,
                    SUBGAME_BUNDLE_ADDR.into(),
                    InitTableData {
                        btn: 0,
                        table_id: 2,
                        sb: 10,
                        bb: 20,
                        table_size: 3,
                        player_lookup: BTreeMap::from([
                            (pb.id(), Player::new(pb.id(), 1000, 0, 0)),
                            (pf.id(), Player::new(pf.id(), 1000, 1, 0)),
                            (pj.id(), Player::new(pj.id(), 1000, 2, 0)),
                        ])
                    }
                )?,
                LaunchSubGame::try_new(
                    3,
                    SUBGAME_BUNDLE_ADDR.into(),
                    InitTableData {
                        btn: 0,
                        table_id: 3,
                        sb: 10,
                        bb: 20,
                        table_size: 3,
                        player_lookup: BTreeMap::from([
                            (pc.id(), Player::new(pc.id(), 1000, 0, 0)),
                            (pg.id(), Player::new(pg.id(), 1000, 1, 0)),
                            (pk.id(), Player::new(pk.id(), 1000, 2, 0)),
                        ])
                    }
                )?,
                LaunchSubGame::try_new(
                    4,
                    SUBGAME_BUNDLE_ADDR.into(),
                    InitTableData {
                        btn: 0,
                        table_id: 4,
                        sb: 10,
                        bb: 20,
                        table_size: 3,
                        player_lookup: BTreeMap::from([
                            (pd.id(), Player::new(pd.id(), 1000, 0, 0)),
                            (ph.id(), Player::new(ph.id(), 1000, 1, 0)),
                            (pl.id(), Player::new(pl.id(), 1000, 2, 0)),
                        ])
                    }
                )?,
            ]
        );

        assert_eq!(mtt.start_time, 1001);
        assert_eq!(mtt.alives, 12);
        assert_eq!(mtt.stage, MttStage::Playing);
        assert_eq!(mtt.ranks.len(), 12);
        assert_eq!(mtt.tables.len(), 4);
        assert_eq!(mtt.table_size, 3);
        assert_eq!(mtt.time_elapsed, 50);
        assert_eq!(mtt.timestamp, effect.timestamp);
        assert_eq!(mtt.blind_info, BlindInfo::default());

        // T3 settles: 2 players out, 1 player alive
        // T1 settles: 1 player will be moved to t3 (smallest)
        let t3_game_result = HoldemBridgeEvent::GameResult {
            table_id: 3,
            settles: vec![
                Settle {
                    id: pc.id(),
                    op: SettleOp::Add(2000),
                },
                Settle {
                    id: pg.id(),
                    op: SettleOp::Sub(1000),
                },
                Settle {
                    id: pk.id(),
                    op: SettleOp::Sub(1000),
                },
            ],
            // checkpoint contains alive players only
            checkpoint: MttTableCheckpoint {
                btn: 0,
                players: vec![MttTablePlayer {
                    id: pc.id(),
                    chips: 3000,
                    table_position: 0,
                }],
            },
        };

        let t1_game_result = HoldemBridgeEvent::GameResult {
            table_id: 1,
            settles: vec![
                Settle {
                    id: pa.id(),
                    op: SettleOp::Add(200),
                },
                Settle {
                    id: pe.id(),
                    op: SettleOp::Sub(100),
                },
                Settle {
                    id: pi.id(),
                    op: SettleOp::Sub(100),
                },
            ],
            checkpoint: MttTableCheckpoint {
                btn: 0,
                players: vec![
                    MttTablePlayer {
                        id: pa.id(),
                        chips: 1200,
                        table_position: 0,
                    },
                    MttTablePlayer {
                        id: pe.id(),
                        chips: 900,
                        table_position: 1,
                    },
                    MttTablePlayer {
                        id: pi.id(),
                        chips: 900,
                        table_position: 2,
                    },
                ],
            },
        };

        let evts = [
            Event::Bridge {
                dest: 3,
                raw: t3_game_result.try_to_vec()?,
            },
            Event::Bridge {
                dest: 3,
                raw: t1_game_result.try_to_vec()?,
            },
        ];

        for evt in evts {
            mtt.handle_event(&mut effect, evt)?;
        }

        assert_eq!(
            effect.bridge_events,
            vec![
                EmitBridgeEvent::try_new(
                    3,
                    HoldemBridgeEvent::Relocate {
                        players: vec![MttTablePlayer { id: pa.id(), chips: 1200, table_position: 0 }],
                    }
                )?,
                EmitBridgeEvent::try_new(
                    1,
                    HoldemBridgeEvent::StartGame {
                        sb: 10,
                        bb: 20,
                        moved_players: vec![pa.id()] // player pa(id) left
                    }
                )?
            ]
        );

        assert_eq!(mtt.tables.len(), 4); // no table gets closed
        assert_eq!(mtt.alives, 10);
        assert_eq!(mtt.stage, MttStage::Playing);

        Ok(())
    }

    #[test]
    fn test_close_table() -> anyhow::Result<()> {
                let mut pa = TestClient::player("pa");
        let mut pb = TestClient::player("pb");
        let mut pc = TestClient::player("pc");
        let mut pd = TestClient::player("pd");
        let mut pe = TestClient::player("pe");
        let mut pf = TestClient::player("pf");
        let mut pg = TestClient::player("pg");
        let mut ph = TestClient::player("ph");
        let mut pi = TestClient::player("pi");
        let mut pj = TestClient::player("pj");
        let mut pk = TestClient::player("pk");
        let mut pl = TestClient::player("pl");
        let players = [
            &mut pa, &mut pb, &mut pc, &mut pd, &mut pe, &mut pf, &mut pg, &mut ph, &mut pi,
            &mut pj, &mut pk, &mut pl,
        ];

        let mut mtt = setup_mtt_state(players)?;
        let evt = Event::GameStart { access_version: 0 };
        let mut effect = Effect::default();
        mtt.handle_event(&mut effect, evt)?;

        assert_eq!(mtt.alives, 12);
        assert_eq!(mtt.stage, MttStage::Playing);
        assert_eq!(mtt.tables.len(), 4);

        // T4 settles: 1 out and 2 alive so one emtpy seats available
        // T3 settles: 2 out and 1 alive, so T3 should be closed and the alive to be moved
        let t3_game_result = HoldemBridgeEvent::GameResult {
            table_id: 3,
            settles: vec![
                Settle {
                    id: pc.id(),
                    op: SettleOp::Add(2000),
                },
                Settle {
                    id: pg.id(),
                    op: SettleOp::Sub(1000),
                },
                Settle {
                    id: pk.id(),
                    op: SettleOp::Sub(1000),
                },
            ],
            // checkpoint contains alive players only
            checkpoint: MttTableCheckpoint {
                btn: 0,
                players: vec![MttTablePlayer {
                    id: pc.id(),
                    chips: 3000,
                    table_position: 0,
                }],
            },
        };

        let t4_game_result = HoldemBridgeEvent::GameResult {
            table_id: 4,
            settles: vec![
                Settle {
                    id: pd.id(),
                    op: SettleOp::Add(500),
                },
                Settle {
                    id: ph.id(),
                    op: SettleOp::Add(500),
                },
                Settle {
                    id: pl.id(),
                    op: SettleOp::Sub(1000),
                },
            ],
            checkpoint: MttTableCheckpoint {
                btn: 0,
                players: vec![
                    MttTablePlayer {
                        id: pd.id(),
                        chips: 1500,
                        table_position: 0,
                    },
                    MttTablePlayer {
                        id: ph.id(),
                        chips: 1500,
                        table_position: 1,
                    },
                ],
            },
        };
        let evts = [
            Event::Bridge {
                dest: 4,
                raw: t4_game_result.try_to_vec()?,
            },
            Event::Bridge {
                dest: 3,
                raw: t3_game_result.try_to_vec()?,
            },
        ];

        for evt in evts {
            mtt.handle_event(&mut effect, evt)?;
        }

        assert_eq!(
            effect.bridge_events,
            vec![
                EmitBridgeEvent::try_new(
                    4,
                    HoldemBridgeEvent::Relocate {
                        players: vec![MttTablePlayer { id: pc.id(), chips: 3000, table_position: 0 }]
                    }
                )?,
                EmitBridgeEvent::try_new(3, HoldemBridgeEvent::CloseTable)?,
            ]
        );

        assert_eq!(mtt.tables.len(), 3); // one table gets closed
        assert_eq!(mtt.stage, MttStage::Playing);
        assert_eq!(mtt.alives, 9);
        Ok(())
    }
}
