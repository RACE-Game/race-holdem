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
    // The maximum number is 9 players at the same time,
    // but never check in ltmtt, it's the launcher's responsibility.
    pub table_size: u8,
    // First time register: xUSDT -> yCHIPS
    // Register again: xUSDT -> zCHIPS
    pub ticket_rules: Vec<TicketRule>,
    pub total_prize: u64,
    // pub blind_rules: Vec<u8>,
    // pub prize_rules: Vec<u8>,
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
            total_prize,
            subgame_bundle,
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
            total_prize,
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
                        self.do_sit_in(effect, &player)?;
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
    fn on_ready(&self, effect: &mut Effect) -> HandleResult<()> {
        effect.info("callback on_ready...");
        effect.set_entry_lock(EntryLock::Closed);
        effect.start_game();

        Ok(())
    }

    // reset game state, swap secret
    fn on_game_start(&mut self, effect: &mut Effect) -> HandleResult<()> {
        effect.info("callback on_game_start...");

        for blind_rule in self.blind_rules.clone() {
            let _ = self.create_table(effect, blind_rule);
        }

        effect.wait_timeout(self.entry_open_time.saturating_sub(effect.timestamp()));
        Ok(())
    }

    fn on_waiting_timeout(&mut self, effect: &mut Effect) -> HandleResult<()> {
        match self.stage {
            LtMttStage::Init => {
                effect.info("callback on_waiting_timeout: stage changed to EntryOpened.");
                effect.wait_timeout(self.entry_close_time.saturating_sub(effect.timestamp()));
                effect.set_entry_lock(EntryLock::Open);
                self.stage = LtMttStage::EntryOpened;
            }

            LtMttStage::EntryOpened => {
                effect.info("callback on_waiting_timeout: stage changed to EntryClosed.");
                effect.wait_timeout(self.settle_time.saturating_sub(effect.timestamp()));
                effect.set_entry_lock(EntryLock::Closed);
                self.stage = LtMttStage::EntryClosed;
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
            self.player_balances.insert(deposit.id(), deposit.balance());

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
            self.apply_chips_change(chips_change)?;
            let new_table = self.tables.get(&table_id).expect("error_table_not_found");

            effect.bridge_event(
                table_id,
                HoldemBridgeEvent::StartGame {
                    sb: new_table.sb,
                    bb: new_table.bb,
                    moved_players: vec![],
                },
            )?;
            effect.checkpoint();

            Ok(())
        } else {
            Ok(())
        }
    }

    fn level_up(&mut self) -> () {}

    fn do_sit_in(&mut self, effect: &mut Effect, player: &LtMttPlayer) -> HandleResult<()> {
        let table_id;
        let mut mtt_table_player = MttTablePlayer::new(
            player.player_id,
            player.chips,
            0,
            MttTablePlayerStatus::SitIn,
        );

        // Sit into lowest sb/bb table directly.
        // Move player to higher sb/bb table if satisfy condition when `game_result`.
        table_id = match self.table_assigns.get(&player.player_id) {
            Some(exist_table_id) => *exist_table_id,
            None => self.find_table_to_sit(player),
        };

        let table_ref = self
            .tables
            .get_mut(&table_id)
            .ok_or(errors::error_table_not_found())?;

        if table_ref.add_player(&mut mtt_table_player) {
            self.table_assigns.insert(player.player_id, table_id);
            let _ = self.create_table_if_target_table_not_enough(effect, table_id);

            effect.bridge_event(
                table_id,
                HoldemBridgeEvent::Relocate {
                    players: vec![mtt_table_player],
                },
            )?;

            effect.info(format!(
                "do_sit_in: user {} sit in table {}.",
                player.player_id, table_id
            ));
            effect.checkpoint();

            Ok(())
        } else {
            Err(errors::error_player_already_on_table())
        }
    }

    fn find_table_to_sit(&mut self, player: &LtMttPlayer) -> usize {
        let matched_blind_rule = match_blind_rule_by_chips(&self.blind_rules, player.chips);
        let mut sorted_tables: Vec<_> = self
            .tables
            .iter()
            .filter(|(_, t)| {
                matched_blind_rule.sb <= t.sb && self.table_size as usize > t.players.len()
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

    fn apply_chips_change(&mut self, chips_change: BTreeMap<u64, ChipsChange>) -> HandleResult<()> {
        for (player_id, change) in chips_change.into_iter() {
            let player = self
                .rankings
                .iter_mut()
                .find(|p| p.player_id == player_id)
                .ok_or(errors::error_player_not_found())?;

            match change {
                ChipsChange::Add(amount) => {
                    player.chips += amount;
                }
                ChipsChange::Sub(amount) => {
                    player.chips -= amount;
                    if player.chips <= 0 {
                        player.status = LtMttPlayerStatus::SatOut;
                    }
                }
            }
        }

        Ok(())
    }

    fn do_settle(&mut self, effect: &mut Effect) -> HandleResult<()> {
        if self.prize_rules.is_empty() {
            self.player_balances.insert(0, 0);
            return Ok(());
        }

        let total_shares: u8 = self.prize_rules.iter().take(self.rankings.len()).sum();
        let prize_share: u64 = self.total_prize / total_shares as u64;

        for (i, ranking) in self.rankings.iter().enumerate() {
            let player_id = ranking.player_id;

            if let Some(rank) = self.prize_rules.get(i) {
                let prize: u64 = prize_share * *rank as u64;
                effect.withdraw(player_id, prize);
            }
        }

        self.player_balances.insert(0, 0);

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
