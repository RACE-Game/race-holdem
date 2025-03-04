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
    ChipsChange, HoldemBridgeEvent, MttTablePlayer, MttTablePlayerStatus, MttTableState,
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

#[derive(Default, BorshSerialize, BorshDeserialize, Debug, Clone)]
pub enum LtMttPlayerStatus {
    #[default]
    SatIn,
    SatOut,
}

#[derive(Default, BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct LtMttPlayer {
    player_id: u64,
    position: u16,
    status: LtMttPlayerStatus,
    chips: u64,
    deposit_history: Vec<u64>,
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
    tables: BTreeMap<usize, MttTableState>,
    table_assigns: BTreeMap<u64, usize>,
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
                        // self.do_sit_in(effect, &player)?;
                        self.rankings.sort_by_key(|ranking| Reverse(ranking.chips));
                    } else {
                        ()
                    }
                }
            }

            Event::Bridge { raw, .. } => {
                let bridge_event = HoldemBridgeEvent::try_parse(&raw)?;

                match bridge_event {
                    game_result @ HoldemBridgeEvent::GameResult { .. } => {
                        self.on_game_result(effect, game_result)?;
                        self.rankings.sort_by_key(|ranking| Reverse(ranking.chips));
                    }
                    _ => return Err(errors::error_invalid_bridge_event()),
                }
            }

            Event::SubGameReady { .. } => {
                effect.checkpoint();
            }

            Event::Custom { sender, raw } => {
                let event: ClientEvent = ClientEvent::try_parse(&raw)?;

                match event {
                    ClientEvent::SitIn => {
                        effect.info("Client event sitin received.");
                        let player = self.find_player_by_id(sender);
                        // Sit into lowest sb/bb table directly.
                        // Move player to higher sb/bb table if satisfy condition when `game_result`.
                        let table_id = match self.table_assigns.get(&player.player_id) {
                            Some(exist_table_id) => *exist_table_id,
                            None => self.find_table_to_sit(1, player.chips),
                        };
                        let sit_players = BTreeMap::from([(player.player_id, player.chips)]);
                        self.do_sit_in(effect, table_id, sit_players)?;
                    }

                    _ => (),
                }
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
        // effect.set_entry_lock(EntryLock::Closed);

        if self.stage == LtMttStage::Init {
            effect.start_game();
        } else {
            if let Some(timeout) = self.calc_timeout_should_send(effect) {
                effect.wait_timeout(timeout);
            }
        }

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
                for blind_rule in self.blind_rules.clone() {
                    let _ = self.create_table(effect, blind_rule);
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
                    status: LtMttPlayerStatus::SatIn,
                    chips: 0,
                    deposit_history: vec![],
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
            chips_change,
            table,
        } = game_result
        {
            effect.info(format!("on_game_result: table_id: {}", table_id));
            let old_table_player_ids: Vec<_> = self
                .tables
                .get(&table_id)
                .unwrap()
                .players
                .clone()
                .iter()
                .map(|p| p.id)
                .collect();
            let new_table_player_ids: Vec<_> = table.players.clone().iter().map(|p| p.id).collect();

            let moved_player_ids: Vec<_> = old_table_player_ids
                .iter()
                .filter(|&p| !new_table_player_ids.contains(p))
                .collect();

            for player_id in moved_player_ids {
                // Remove player from table_assigns
                self.table_assigns.remove(player_id);
                // Set ltmttplayerstatus to sitout
                for ranking in self.rankings.iter_mut() {
                    if ranking.player_id == *player_id {
                        ranking.status = LtMttPlayerStatus::SatOut;
                    }
                }
            }

            self.tables.insert(table_id, table);
            self.apply_chips_change(effect, chips_change)?;
            self.level_up(effect, table_id)?;

            effect.checkpoint();
            Ok(())
        } else {
            Ok(())
        }
    }

    fn level_up(&mut self, effect: &mut Effect, table_id: usize) -> HandleResult<()> {
        let origin_table = self.tables.get_mut(&table_id).unwrap();
        let cur_blind_rule = self
            .blind_rules
            .iter()
            .find(|br| br.sb == origin_table.sb)
            .unwrap();

        if let Some(max_chips) = cur_blind_rule.max_chips {
            let mut level_up_player_ids: Vec<u64> = vec![];
            for player in origin_table.players.iter() {
                if player.chips > max_chips {
                    level_up_player_ids.push(player.id);
                }
            }
            let origin_player_num = origin_table.players.len();
            let level_up_player_num = level_up_player_ids.len();

            // to moved player num >= 2
            // origin table player num >= 2, or eq 0 after moved.
            if level_up_player_ids.len() >= 2
                && (origin_player_num == level_up_player_num
                    || (origin_player_num - level_up_player_num) >= 2)
            {
                // origin table
                for &up_player_id in level_up_player_ids.iter() {
                    self.table_assigns.remove(&up_player_id);

                    let mut origin_new_players = vec![];
                    for p in origin_table.players.iter() {
                        if p.id != up_player_id {
                            origin_new_players.push(p.clone());
                        }
                    }
                    origin_table.players = origin_new_players;
                }

                effect.bridge_event(
                    table_id,
                    HoldemBridgeEvent::StartGame {
                        sb: origin_table.sb,
                        bb: origin_table.bb,
                        moved_players: level_up_player_ids.clone(),
                    },
                )?;

                // up level table
                let next_blind_rule = self
                    .blind_rules
                    .iter()
                    .find(|br| origin_table.sb < br.sb)
                    .unwrap();
                let next_max_chips = next_blind_rule.max_chips.unwrap_or(u64::max_value());
                let to_sit_table_id = self.find_table_to_sit(level_up_player_num, next_max_chips);
                let relocate_players: BTreeMap<u64, u64> = level_up_player_ids
                    .into_iter()
                    .map(|pid| {
                        (
                            pid,
                            self.rankings
                                .iter()
                                .find(|r| r.player_id == pid)
                                .unwrap()
                                .chips,
                        )
                    })
                    .collect();

                self.do_sit_in(effect, to_sit_table_id, relocate_players)?;
                return Ok(());
            }
        }

        effect.bridge_event(
            table_id,
            HoldemBridgeEvent::StartGame {
                sb: origin_table.sb,
                bb: origin_table.bb,
                moved_players: vec![],
            },
        )?;

        return Ok(());
    }

    /// Used with `find_table_to_sit`, which ensure multi player sit into one table.
    /// `do_sit_in` handle all the state then.
    ///
    /// # Params
    ///
    /// players: BTreeMap<player_id, chips>
    ///
    fn do_sit_in(
        &mut self,
        effect: &mut Effect,
        table_id: usize,
        players: BTreeMap<u64, u64>,
    ) -> HandleResult<()> {
        let table_ref = self
            .tables
            .get_mut(&table_id)
            .ok_or(errors::error_table_not_found())?;

        let mut mtt_table_players = vec![];
        for (player_id, chips) in players {
            let mtt_table_player =
                MttTablePlayer::new(player_id, chips, 0, MttTablePlayerStatus::SitIn);
            let ranking = self
                .rankings
                .iter_mut()
                .find(|rank| rank.player_id == player_id)
                .unwrap();
            ranking.status = LtMttPlayerStatus::SatIn;
            mtt_table_players.push(mtt_table_player);
        }

        for mtt_player in mtt_table_players.iter_mut() {
            if table_ref.players.len() >= self.table_size as usize {
                return Err(errors::error_table_is_full());
            } else if !table_ref.add_player(mtt_player) {
                return Err(errors::error_player_already_on_table());
            } else {
                self.table_assigns.insert(mtt_player.id, table_id);
                effect.info(format!(
                    "ltmtt do_sit_in: user[{}] sit in table[{}].",
                    mtt_player.id, table_id
                ));
            }
        }

        effect.bridge_event(
            table_id,
            HoldemBridgeEvent::Relocate {
                players: mtt_table_players,
            },
        )?;

        // Ensure there's always an empty table for each level.
        self.create_table_if_target_table_not_enough(effect, table_id)?;
        effect.checkpoint();

        Ok(())
    }

    /// find_table_to_sit
    /// find the table which satisfy
    ///
    /// # Parmas
    ///
    /// - `self`
    /// - `seats`: how many seats the caller needs
    /// - `chips`: the MAX chips of these players
    ///
    /// # Return
    ///
    /// - table_id: usize
    ///
    fn find_table_to_sit(&mut self, seats: usize, player_chips: u64) -> usize {
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
        chips_change: BTreeMap<u64, ChipsChange>,
    ) -> HandleResult<()> {
        for (player_id, change) in chips_change.into_iter() {
            let player = self
                .rankings
                .iter_mut()
                .find(|p| p.player_id == player_id)
                .ok_or(errors::error_player_not_found())?;

            match change {
                ChipsChange::Add(amount) => {
                    effect.info(format!(
                        "chips_changes: Player[{}] chips: {} add {}",
                        player_id, player.chips, amount
                    ));
                    player.chips += amount;
                }
                ChipsChange::Sub(amount) => {
                    effect.info(format!(
                        "chips_changes: Player[{}] chips: {} sub {}",
                        player_id, player.chips, amount
                    ));
                    player.chips -= amount;
                    if player.chips <= 0 {
                        player.status = LtMttPlayerStatus::SatOut;
                        self.table_assigns.remove(&player.player_id);
                    }
                }
            }
        }

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

    #[allow(unused)]
    fn find_or_create_table(
        &mut self,
        effect: &mut Effect,
        player: &LtMttPlayer,
    ) -> HandleResult<usize> {
        // Sort tables, order by sb asc, players_num asc
        // Traverse sorted_tables, the first one is the best table fit current player.
        let mut sorted_tables: Vec<_> = self.tables.iter().collect();
        sorted_tables.sort_by(|(_, a), (_, b)| {
            a.sb.cmp(&b.sb)
                .then_with(|| a.players.len().cmp(&b.players.len()))
        });

        let matched_blind_rule = match_blind_rule_by_chips(&self.blind_rules, player.chips);

        if let Some((&id, _)) = sorted_tables.iter().find(|(_id, table)| {
            matched_blind_rule.sb <= table.sb && table.players.len() < self.table_size as _
        }) {
            Ok(id)
        } else {
            let table_id = self.create_table(effect, matched_blind_rule.clone())?;
            Ok(table_id)
        }
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

    #[test]
    fn test() {
        let effect = [
            0, 0, 0, 0, 0, 22, 76, 197, 244, 147, 1, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0,
            0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 228, 0, 0, 0, 54, 191, 170, 244, 147, 1, 0, 0, 150, 169,
            171, 244, 147, 1, 0, 0, 22, 76, 197, 244, 147, 1, 0, 0, 9, 6, 0, 0, 0, 1, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 152, 58, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0,
            0, 224, 103, 53, 0, 0, 0, 0, 0, 144, 58, 28, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0,
            128, 240, 250, 2, 0, 0, 0, 0, 224, 112, 114, 0, 0, 0, 0, 0, 0, 32, 161, 7, 0, 0, 0, 0,
            0, 240, 73, 2, 0, 0, 0, 0, 0, 0, 224, 103, 53, 0, 0, 0, 0, 0, 144, 58, 28, 0, 0, 0, 0,
            0, 0, 128, 240, 250, 2, 0, 0, 0, 0, 224, 112, 114, 0, 0, 0, 0, 0, 16, 39, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 40, 0, 0, 0, 114, 97, 99, 101, 104, 111, 108, 100, 101,
            109, 116, 97, 114, 103, 101, 116, 114, 97, 99, 101, 104, 111, 108, 100, 101, 109, 108,
            116, 109, 116, 116, 116, 97, 98, 108, 101, 119, 97, 115, 109, 2, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0,
        ];
        let event = [12];

        let mut effect = Effect::try_from_slice(&effect).unwrap();
        let event = Event::try_from_slice(&event).unwrap();

        let mut ltmtt = effect.__handler_state::<LtMtt>();
        ltmtt.handle_event(&mut effect, event).unwrap();
    }

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
