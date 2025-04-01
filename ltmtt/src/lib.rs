mod errors;

use std::vec;
use std::{cmp::Reverse, collections::BTreeMap};

// use crate::errors::error_leave_not_allowed;
use borsh::{BorshDeserialize, BorshSerialize};
// use race_api::engine::GameHandler;
use race_api::{
    prelude::*,
    types::{EntryLock, GameDeposit},
};

use race_holdem_mtt_base::{
    HoldemBridgeEvent, MttTablePlayer, MttTableSitin, MttTableState, PlayerResult,
    PlayerResultStatus, DEFAULT_TIME_CARDS,
};
use race_proc_macro::game_handler;

#[derive(Default, BorshSerialize, BorshDeserialize)]
pub struct LtMttAccountData {
    // Time unit is millis
    pub entry_open_time: u64,
    pub entry_close_time: u64,
    pub settle_time: u64,
    // Players on table, but never check in ltmtt, it's the launcher's responsibility.
    pub table_size: u8,
    // First time register: xUSDT -> yCHIPS
    // Register again: xUSDT -> zCHIPS
    pub ticket_rules: Vec<TicketRule>,
    pub rake: u16,
    // pub blind_rules: Vec<u8>,
    pub prize_rules: Vec<u8>,
    pub subgame_bundle: String,
}

#[derive(Default, BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct TicketRule {
    // `None` in `deposit_times` means any other time.  `0` represents the first time.
    deposit_times: Option<usize>,
    deposit_amount: u64,
    chips: u64,
}

impl TicketRule {
    fn is_match(&self, times: usize, amount: u64) -> bool {
        self.deposit_times.map(|t| t == times).unwrap_or(true) && self.deposit_amount == amount
    }
}

fn default_ticket_rules() -> Vec<TicketRule> {
    vec![
        TicketRule {
            deposit_times: Some(0),
            deposit_amount: 0,
            chips: 15_000,
        },
        TicketRule {
            deposit_times: Some(0),
            deposit_amount: 3_500_000,
            chips: 1_850_000,
        },
        TicketRule {
            deposit_times: Some(0),
            deposit_amount: 50_000_000,
            chips: 7_500_000,
        },
        TicketRule {
            deposit_times: None,
            deposit_amount: 500_000,
            chips: 150_000,
        },
        TicketRule {
            deposit_times: None,
            deposit_amount: 3_500_000,
            chips: 1_850_000,
        },
        TicketRule {
            deposit_times: None,
            deposit_amount: 50_000_000,
            chips: 7_500_000,
        },
    ]
}

// pub struct PrizeRules {}

#[derive(Default, BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct BlindRule {
    // `None` represents no chips upper limit.
    max_chips: Option<u64>,
    sb: u64,
    bb: u64,
    ante: u64,
}

fn match_blind_rule_by_chips(rules: &Vec<BlindRule>, chips: u64) -> &BlindRule {
    // BlindRules MUST ensures sorted by max_chips asc, because the client can pass custom rules.
    // Sort Vec<BlindRule> in init_state handler.
    rules
        .iter()
        .find(|rule| chips <= rule.max_chips.unwrap_or(u64::max_value()))
        .expect("BlindRules Error")
}

fn default_blind_rules() -> Vec<BlindRule> {
    vec![
        BlindRule {
            max_chips: Some(150_000),
            sb: 50,
            bb: 100,
            ante: 0,
        },
        BlindRule {
            max_chips: Some(300_000),
            sb: 100,
            bb: 200,
            ante: 0,
        },
        BlindRule {
            max_chips: Some(400_000),
            sb: 200,
            bb: 400,
            ante: 0,
        },
        BlindRule {
            max_chips: Some(1_000_000),
            sb: 500,
            bb: 1000,
            ante: 0,
        },
        BlindRule {
            max_chips: Some(2_000_000),
            sb: 1000,
            bb: 2000,
            ante: 0,
        },
        BlindRule {
            max_chips: Some(4_000_000),
            sb: 2000,
            bb: 4000,
            ante: 0,
        },
        BlindRule {
            max_chips: Some(8_000_000),
            sb: 4000,
            bb: 8000,
            ante: 0,
        },
        BlindRule {
            max_chips: Some(10_000_000),
            sb: 5000,
            bb: 10_000,
            ante: 0,
        },
        BlindRule {
            max_chips: Some(20_000_000),
            sb: 10_000,
            bb: 20_000,
            ante: 0,
        },
        BlindRule {
            max_chips: None,
            sb: 20_000,
            bb: 40_000,
            ante: 0,
        },
    ]
}

#[derive(Default, BorshSerialize, BorshDeserialize, Debug, Clone, PartialEq)]
pub enum LtMttPlayerStatus {
    #[default]
    // player not join in game.
    Out,
    // player intend to sitin a table.
    Pending,
    // player is playing in a table.
    Playing,
}

#[derive(Default, BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct LtMttPlayer {
    player_id: u64,
    position: u16,
    status: LtMttPlayerStatus,
    chips: u64,
    deposit_history: Vec<u64>,
    time_cards: u8,
}

#[derive(Default, PartialEq, BorshSerialize, BorshDeserialize, Debug, Clone, Copy)]
pub enum LtMttStage {
    #[default]
    Init,
    EntryOpened,
    EntryClosed,
    Settled,
}

#[derive(BorshSerialize, BorshDeserialize, Debug, PartialEq, Clone)]
pub enum ClientEvent {
    SitIn,
    SitOut,
}

impl CustomEvent for ClientEvent {}

#[game_handler]
#[derive(Default, BorshSerialize, BorshDeserialize, Debug)]
pub struct LtMtt {
    entry_open_time: u64,
    entry_close_time: u64,
    settle_time: u64,
    table_size: u8,
    ticket_rules: Vec<TicketRule>,
    total_prize: u64,
    prize_rules: Vec<u8>,
    rake: u16,
    blind_rules: Vec<BlindRule>,
    subgame_bundle: String,
    /// belows are ltmtt self hold fields
    stage: LtMttStage,
    rankings: Vec<LtMttPlayer>,
    // table_id -> table_state
    tables: BTreeMap<usize, MttTableState>,
    // player_id -> table_id
    table_assigns: BTreeMap<u64, usize>,
    // players who request sit in a table but not receive a confirmation.
    table_assigns_pending: BTreeMap<u64, usize>,
    // theme: Option<String>,
    player_balances: BTreeMap<u64, u64>,
}

impl GameHandler for LtMtt {
    fn balances(&self) -> Vec<PlayerBalance> {
        // self.player_balances
        //     .iter()
        //     .map(|(player_id, balance)| PlayerBalance { player_id, balance });
        let sum = self.player_balances.values().sum();

        vec![PlayerBalance {
            player_id: 0,
            balance: sum,
        }]
    }

    fn init_state(init_account: InitAccount) -> HandleResult<Self> {
        let LtMttAccountData {
            entry_open_time,
            entry_close_time,
            settle_time,
            table_size,
            mut ticket_rules,
            rake,
            prize_rules,
            subgame_bundle,
            ..
        } = init_account.data()?;

        if ticket_rules.is_empty() {
            ticket_rules = default_ticket_rules();
        }

        // If params invalid, avoid create LtMtt game.
        if entry_open_time > entry_close_time || entry_close_time > settle_time {
            return Err(HandleError::MalformedGameAccountData);
        }

        let blind_rules = default_blind_rules();
        let player_balances = BTreeMap::from([(0, 0)]);

        let state = Self {
            entry_open_time,
            entry_close_time,
            settle_time,
            table_size,
            ticket_rules,
            rake,
            prize_rules,
            subgame_bundle,
            blind_rules,
            player_balances,
            ..Default::default()
        };

        Ok(state)
    }

    fn handle_event(&mut self, effect: &mut Effect, event: Event) -> HandleResult<()> {
        match event {
            Event::Ready => self.on_ready(effect)?,
            Event::GameStart => self.on_game_start(effect)?,
            Event::WaitingTimeout => self.on_waiting_timeout(effect)?,
            // Only first register will receive this event.
            // This event only add player to rankings board.
            Event::Join { players } => self.on_join(effect, players)?,
            // If player loss all chips, he can register again, then ltmtt will receive deposit event.
            // If deposit succeed, sit in to a table.
            Event::Deposit { deposits } => {
                for deposit in deposits {
                    if let Some(_player) = self.on_deposit(effect, &deposit)? {
                        self.rankings.sort_by_key(|ranking| Reverse(ranking.chips));
                    } else {
                        ()
                    }
                }
            }

            Event::Custom { sender, raw } => {
                let event: ClientEvent = ClientEvent::try_parse(&raw)?;

                match event {
                    // player first join to play
                    ClientEvent::SitIn => {
                        effect.info(format!("Player[{}] sitin custom event.", sender));
                        let player = self.find_player_by_id(sender);
                        // Sit into lowest sb/bb table directly.
                        // Move player to higher sb/bb table if satisfy condition when `game_result`.
                        let table_id = match self.table_assigns.get(&player.player_id) {
                            Some(exist_table_id) => *exist_table_id,
                            None => self.find_table_to_sit(1, player.chips),
                        };
                        // send a bridge event to subgame, waiting `HoldemBridgeEvent::SitinResult` from subgame.
                        // when received the response, update self state.
                        let sitin = MttTableSitin::new(player.player_id, player.chips, player.time_cards);
                        effect.bridge_event(
                            table_id,
                            HoldemBridgeEvent::SitinPlayers {
                                sitins: vec![sitin],
                            },
                        )?;
                        self.table_assigns_pending.insert(player.player_id, table_id);
                    }

                    _ => (),
                }
            }

            Event::Bridge { raw, .. } => {
                let bridge_event = HoldemBridgeEvent::try_parse(&raw)?;

                match bridge_event {
                    game_result @ HoldemBridgeEvent::GameResult { .. } => {
                        self.on_game_result(effect, game_result)?;
                        effect.checkpoint();
                    }

                    _ => return Err(errors::error_invalid_bridge_event()),
                }
            }

            Event::SubGameReady { .. } => {
                effect.checkpoint();
            }

            _ => (),
        }

        Ok(())
    }
}

impl LtMtt {
    fn calc_timeout_should_send(&self, effect: &mut Effect) -> Option<u64> {
        match self.stage {
            LtMttStage::Init => Some(self.entry_open_time.saturating_sub(effect.timestamp())),
            LtMttStage::EntryOpened => {
                Some(self.entry_close_time.saturating_sub(effect.timestamp()))
            }
            LtMttStage::EntryClosed => Some(self.settle_time.saturating_sub(effect.timestamp())),
            LtMttStage::Settled => None,
        }
    }

    fn on_ready(&mut self, effect: &mut Effect) -> HandleResult<()> {
        effect.info("callback on_ready...");

        if !self.table_assigns_pending.is_empty() {
            let mut grouped_players: BTreeMap<usize, Vec<u64>> = BTreeMap::new();

            for (player_id, table_id) in self.table_assigns_pending.clone() {
                grouped_players
                    .entry(table_id)
                    .or_insert_with(Vec::new)
                    .push(player_id)
            }

            for (table_id, player_ids) in grouped_players {
                let sitins: Vec<MttTableSitin> = player_ids
                    .iter()
                    .map(|id| {
                        let player = self.find_player_by_id(*id);
                        MttTableSitin::new(player.player_id, player.chips, player.time_cards)
                    })
                    .collect();

                effect.bridge_event(table_id, HoldemBridgeEvent::SitinPlayers { sitins })?;
            }
        }

        effect.start_game();
        Ok(())
    }

    // Reset game state, swap secret
    fn on_game_start(&mut self, effect: &mut Effect) -> HandleResult<()> {
        effect.info("callback on_game_start...");
        // Implicit the stage is LtMttStage::Init
        if let Some(timeout) = self.calc_timeout_should_send(effect) {
            effect.wait_timeout(timeout);
        }

        Ok(())
    }

    fn on_waiting_timeout(&mut self, effect: &mut Effect) -> HandleResult<()> {
        match self.stage {
            LtMttStage::Init => {
                effect.info("callback on_waiting_timeout: stage changed to EntryOpened.");
                // Prepare tables if empty, for recovering condition
                if self.tables.is_empty() {
                    for blind_rule in self.blind_rules.clone() {
                        let _ = self.create_table(effect, blind_rule);
                    }
                }

                effect.set_entry_lock(EntryLock::Open);
                self.stage = LtMttStage::EntryOpened;

                if let Some(timeout) = self.calc_timeout_should_send(effect) {
                    effect.wait_timeout(timeout);
                }
            }

            LtMttStage::EntryOpened => {
                effect.info("callback on_waiting_timeout: stage changed to EntryClosed.");
                effect.set_entry_lock(EntryLock::Closed);
                self.stage = LtMttStage::EntryClosed;

                if let Some(timeout) = self.calc_timeout_should_send(effect) {
                    effect.wait_timeout(timeout);
                }
            }

            LtMttStage::EntryClosed => {
                effect.info("callback on_waiting_timeout: stage changed to Settled.");
                self.do_settle(effect)?;
                self.stage = LtMttStage::Settled;
            }

            _ => {}
        }

        Ok(())
    }

    fn on_join(&mut self, _effect: &mut Effect, new_players: Vec<GamePlayer>) -> HandleResult<()> {
        // There's no need to re-sort rankings because new joined user is the last, its stable ranking.
        if self.stage == LtMttStage::EntryOpened {
            for player in new_players {
                // Don't push player into rankings vector again if already registered.
                if self.rankings.iter().any(|p| p.player_id == player.id()) {
                    continue;
                }

                let ltmtt_player = LtMttPlayer {
                    player_id: player.id(),
                    position: player.position(),
                    status: LtMttPlayerStatus::Out,
                    chips: 0,
                    deposit_history: vec![],
                    time_cards: DEFAULT_TIME_CARDS,
                };

                self.rankings.push(ltmtt_player);
            }
        }

        Ok(())
    }

    fn on_deposit(
        &mut self,
        effect: &mut Effect,
        deposit: &GameDeposit,
    ) -> HandleResult<Option<LtMttPlayer>> {
        if self.stage == LtMttStage::EntryOpened {
            let Some(player) = self
                .rankings
                .iter_mut()
                .find(|p| p.player_id == deposit.id())
            else {
                effect.info(format!(
                    "on_deposit: Not found user {} in rankings.",
                    deposit.id()
                ));
                effect.reject_deposit(&deposit)?;
                return Ok(None);
            };

            let Some(ticket_rule) = self
                .ticket_rules
                .iter()
                .find(|tr| tr.is_match(player.deposit_history.len(), deposit.balance()))
            else {
                effect.info(format!(
                    "on_deposit: Not found matched TicketRule for {} deposit: {}.",
                    deposit.id(),
                    deposit.balance()
                ));
                effect.reject_deposit(&deposit)?;
                return Ok(None);
            };

            player.chips = ticket_rule.chips;
            player.deposit_history.push(deposit.balance());
            player.time_cards = DEFAULT_TIME_CARDS;

            let rake = deposit.balance() * 10 / 1000;
            let prize = deposit.balance() - rake;
            self.total_prize += prize;
            self.player_balances.insert(deposit.id(), prize);
            effect.transfer(rake);

            effect.info(format!(
                "on_deposit: User {} deposit {}.",
                deposit.id(),
                deposit.balance()
            ));
            effect.accept_deposit(&deposit)?;

            Ok(Some(player.clone()))
        } else {
            effect.info(format!(
                "on_deposit: Stage not satisfied user {}.",
                deposit.id()
            ));
            effect.reject_deposit(&deposit)?;
            return Ok(None);
        }
    }

    fn on_game_result(
        &mut self,
        effect: &mut Effect,
        game_result: HoldemBridgeEvent,
    ) -> HandleResult<()> {
        if let HoldemBridgeEvent::GameResult {
            hand_id: _,
            table_id,
            player_results,
            table,
        } = game_result
        {
            effect.info(format!("on_game_result: table_id: {}", table_id));
            // Update table state first
            self.tables.insert(table_id, table);
            self.apply_chips_change(effect, &player_results)?;

            for player in player_results {
                if player.status == PlayerResultStatus::Normal {
                    // update current players as `LtMttPlayerStatus::Playing`
                    self.update_table_player(effect, player.player_id, Some(table_id))?;
                } else {
                    // remove moved players, and marked as `LtMttPlayerStatus::Out`
                    self.update_table_player(effect, player.player_id, None)?;
                }
            }

            self.start_next_game(effect, table_id)?;
            Ok(())
        } else {
            Ok(())
        }
    }

    /// Origin table should send start game event,
    /// when received sitresult, confirm all the level up players,
    /// then for level up players find their new table.
    fn start_next_game(&mut self, effect: &mut Effect, table_id: usize) -> HandleResult<()> {
        let origin_table = self.tables.get(&table_id).unwrap();
        let cur_blind_rule = self
            .blind_rules
            .iter()
            .find(|br| br.sb == origin_table.sb)
            .unwrap();

        if let Some(max_chips) = cur_blind_rule.max_chips {
            let mut level_up_players: Vec<MttTablePlayer> = vec![];
            for player in origin_table.players.iter() {
                if player.chips > max_chips {
                    level_up_players.push(player.clone());
                }
            }

            let origin_player_num = origin_table.players.len();
            let level_up_player_num = level_up_players.len();
            effect.info(format!(
                "on_game_result: origin players: {}, level up players: {}",
                origin_player_num, level_up_player_num
            ));

            // To moved player num >= 2
            // Origin table player num >= 2, or eq 0 after moved.
            // TODO: if all players moved from origin table, maybe should close table.
            if level_up_player_num >= 2
                && (origin_player_num == level_up_player_num
                    || (origin_player_num - level_up_player_num) >= 2)
            {
                let up_ids = level_up_players.iter().map(|p| p.id).collect();
                effect.bridge_event(
                    table_id,
                    HoldemBridgeEvent::StartGame {
                        sb: origin_table.sb,
                        bb: origin_table.bb,
                        sitout_players: up_ids,
                    },
                )?;

                let max_chips = level_up_players.iter().max_by_key(|p| p.chips).unwrap();
                let to_table_id = self.find_table_to_sit(level_up_player_num, max_chips.chips);

                let mut sitins = vec![];
                for player in level_up_players.iter() {
                    for ranking in self.rankings.iter_mut() {
                        if ranking.player_id == player.id {
                            ranking.status = LtMttPlayerStatus::Pending;
                        }
                    }
                    sitins.push(MttTableSitin::new(player.id, player.chips, player.time_cards));
                    self.table_assigns_pending.insert(player.id, to_table_id);
                }
                effect.bridge_event(to_table_id, HoldemBridgeEvent::SitinPlayers { sitins })?;

                return Ok(());
            }
        }

        effect.bridge_event(
            table_id,
            HoldemBridgeEvent::StartGame {
                sb: origin_table.sb,
                bb: origin_table.bb,
                sitout_players: vec![],
            },
        )?;

        return Ok(());
    }

    #[allow(unused)]
    fn match_table_for_pending_users(&mut self, effect: &mut Effect) -> HandleResult<()> {
        let pending_players: Vec<_> = self
            .rankings
            .iter()
            // .filter(|r| r.status == LtMttPlayerStatus::Pending())
            .collect();

        // It's a simple match rule, do sit in one by one.
        // For some optimizations, should grouped all pending players.
        for p in pending_players {
            let to_table_id = self.find_table_to_sit(1, p.chips);
            let sitin = MttTableSitin::new(p.player_id, p.chips, p.time_cards);
            effect.bridge_event(
                to_table_id,
                HoldemBridgeEvent::SitinPlayers {
                    sitins: vec![sitin],
                },
            )?;
        }

        Ok(())
    }

    /// Internal update helper function, ignore its result.
    /// Update `tables` and `table_assigns` after receive a confirm bridge event from sub game.
    ///
    /// if table_id None, represents sit out the table, otherwise sit in the table.
    fn update_table_player(
        &mut self,
        effect: &mut Effect,
        player_id: u64,
        table_id: Option<usize>,
    ) -> HandleResult<()> {
        match table_id {
            Some(table_id) => {
                let player = self.find_player_by_id(player_id);
                // remove from origin table if exists
                if let Some(origin_table_id) = self.table_assigns.get(&player_id) {
                    let table = self.tables.get_mut(origin_table_id).unwrap();
                    table.remove_player(player_id);
                    self.table_assigns.remove(&player_id);
                }
                // add into current table
                if let Some(table) = self.tables.get_mut(&table_id) {
                    let mut table_player = MttTablePlayer::new(player_id, player.chips, 0, player.time_cards);
                    table.add_player(&mut table_player);
                    self.table_assigns.insert(player_id, table_id);
                    self.create_table_if_target_table_not_enough(effect, table_id)?;
                } else {
                    return Err(errors::error_table_not_found());
                }

                for ranking in self.rankings.iter_mut() {
                    if ranking.player_id == player_id {
                        ranking.status = LtMttPlayerStatus::Playing;
                    }
                }
            }

            None => {
                if let Some(table_id) = self.table_assigns.get(&player_id) {
                    if let Some(table) = self.tables.get_mut(table_id) {
                        table.remove_player(player_id);
                        self.table_assigns.remove(&player_id);
                    }
                }

                for ranking in self.rankings.iter_mut() {
                    if ranking.player_id == player_id {
                        ranking.status = LtMttPlayerStatus::Out;
                    }
                }
            }
        }

        self.table_assigns_pending.remove(&player_id);
        Ok(())
    }

    /// find_table_to_sit
    /// find the table which satisfy
    ///
    /// # Params
    ///
    /// - `self`
    /// - `seats`: how many seats the caller needs
    /// - `chips`: the MAX chips of these players
    ///
    /// # Return
    ///
    /// - table_id: usize
    ///
    fn find_table_to_sit(&self, seats: usize, player_chips: u64) -> usize {
        let matched_blind_rule = match_blind_rule_by_chips(&self.blind_rules, player_chips);
        let mut sorted_tables: Vec<_> = self
            .tables
            .iter()
            .filter(|(_, t)| {
                matched_blind_rule.sb <= t.sb && t.players.len() + seats <= self.table_size as usize
            })
            .collect();

        // sb/bb asc, player_num desc
        sorted_tables.sort_by(|(_, a), (_, b)| {
            a.sb.cmp(&b.sb)
                .then_with(|| b.players.len().cmp(&a.players.len()))
        });

        let (&id, _) = sorted_tables.first().unwrap();
        return id;
    }

    fn apply_chips_change(
        &mut self,
        effect: &mut Effect,
        player_results: &[PlayerResult],
    ) -> HandleResult<()> {
        for player_result in player_results {
            let player = self
                .rankings
                .iter_mut()
                .find(|p| p.player_id == player_result.player_id)
                .ok_or(errors::error_player_not_found())?;

            effect.info(format!(
                "chips_changes: Player[{}] chips: {}",
                player_result.player_id, player_result.chips
            ));

            player.chips = player_result.chips;
        }

        self.rankings.sort_by_key(|ranking| Reverse(ranking.chips));
        Ok(())
    }

    fn do_settle(&mut self, effect: &mut Effect) -> HandleResult<()> {
        // if no any player join in.
        if self.rankings.is_empty() {
            return Ok(());
        }

        let total_shares: u8 = self.prize_rules.iter().take(self.rankings.len()).sum();
        let prize_share: u64 = self.total_prize / total_shares as u64;
        self.player_balances.insert(0, 0);

        for (i, ranking) in self.rankings.iter().enumerate() {
            let player_id = ranking.player_id;

            if let Some(rule) = self.prize_rules.get(i) {
                let prize: u64 = prize_share * *rule as u64;
                effect.info(format!("do_settle: Player[{}] win {}.", player_id, prize,));
                effect.withdraw(player_id, prize);
            }
        }

        Ok(())
    }

    fn find_player_by_id(&self, player_id: u64) -> LtMttPlayer {
        self.rankings
            .iter()
            .find(|p| p.player_id == player_id)
            .unwrap()
            .clone()
    }

    /// Ensure each blind rule's table has one more empty table.
    fn create_table_if_target_table_not_enough(
        &mut self,
        effect: &mut Effect,
        table_id: usize,
    ) -> HandleResult<()> {
        let table = self.tables.get(&table_id).unwrap();
        let target_tables: Vec<&MttTableState> =
            self.tables.values().filter(|t| t.sb == table.sb).collect();

        if !target_tables.iter().any(|t| t.players.len() == 0) {
            let fake_blind_rule = BlindRule {
                max_chips: Some(0),
                sb: table.sb,
                bb: table.bb,
                ante: 0,
            };
            let _ = self.create_table(effect, fake_blind_rule);
        }

        Ok(())
    }

    fn create_table(&mut self, effect: &mut Effect, blind_rule: BlindRule) -> HandleResult<usize> {
        let table_id = self.tables.iter().map(|(id, _)| *id).max().unwrap_or(0) + 1;
        let BlindRule {
            max_chips: _,
            sb,
            bb,
            ante: _,
        } = blind_rule;

        let table = MttTableState {
            table_id,
            btn: 0,
            sb,
            bb,
            players: Vec::new(),
            next_game_start: 0,
            hand_id: 0,
        };

        effect.launch_sub_game(self.subgame_bundle.clone(), self.table_size as _, &table)?;
        self.tables.insert(table_id, table);

        Ok(table_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use borsh::BorshDeserialize;
    use race_api::{effect::Effect, event::Event};

    // use std::time::SystemTime;
    // use super::*;
    // use race_test::prelude::*;

    // #[test]
    // fn test_join() -> anyhow::Result<()> {
    //     let mut state = LtMtt::default();
    //     let mut effect = Effect::default();
    //     let event = Event::Join {
    //         players: vec![GamePlayer::new(1, 100)],
    //     };
    //     state.handle_event(&mut effect, event)?;
    //     assert_eq!(state.rankings.len(), 1);
    //     assert_eq!(effect.wait_timeout, Some(5000));

    //     Ok(())
    // }

    // #[test]
    // fn test_with_account() -> anyhow::Result<()> {
    //     let mut transactor = TestClient::transactor("server");
    //     let mut alice = TestClient::player("Alice");
    //     // let mut bob = TestClient::player("Bob");
    //     // alice.custom_event(custom_event::Game::Ready);
    //     let ts = SystemTime::now()
    //         .duration_since(SystemTime::UNIX_EPOCH)?
    //         .as_millis() as u64;

    //     let (mut context, _) = TestContextBuilder::default()
    //         .set_transactor(&mut transactor)
    //         .with_data(LtMttAccountData {
    //             start_time: ts,
    //             end_time: ts + 60 * 60 * 1000,
    //             table_size: 2,
    //             start_chips: 100,
    //             prize_rules: vec![100, 50, 30],
    //             theme: None,
    //             subgame_bundle: "subgame".to_string(),
    //         })
    //         .with_max_players(10)
    //         .build_with_init_state::<LtMtt>()?;

    //     let event = context.join(&mut alice, 100);
    //     context.handle_event(&event)?;

    //     {
    //         let state = context.state();
    //         assert_eq!(state.rankings.len(), 1);
    //     }

    //     Ok(())
    // }
}
