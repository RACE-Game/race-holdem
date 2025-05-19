//! Game state machine (or handler) of Holdem: the core of this lib.
use race_api::prelude::*;
use std::collections::BTreeMap;
use std::mem::take;

use crate::account::HoldemAccount;
use crate::errors;
use crate::essential::{
    ActingPlayer, AwardPot, Display, GameEvent, GameMode, HoldemStage, InternalPlayerJoin, Player,
    PlayerResult, PlayerStatus, Pot, Street, ACTION_TIMEOUT_AFK, ACTION_TIMEOUT_POSTFLOP,
    ACTION_TIMEOUT_PREFLOP, ACTION_TIMEOUT_RIVER, ACTION_TIMEOUT_TURN, MAX_ACTION_TIMEOUT_COUNT,
    TIME_CARD_EXTRA_SECS, WAIT_TIMEOUT_DEFAULT,
};
use crate::evaluator::{compare_hands, create_cards, evaluate_cards, PlayerHand};
use crate::hand_history::{BlindBet, BlindType, HandHistory, PlayerAction, Showdown};

// Holdem: the game state
#[derive(BorshSerialize, BorshDeserialize, Default, Debug, PartialEq, Clone)]
pub struct Holdem {
    pub hand_id: usize,
    pub deck_random_id: RandomId,
    pub max_deposit: u64,
    pub sb: u64,
    pub bb: u64,
    pub ante: u64,
    pub min_raise: u64,
    pub btn: usize,
    pub rake: u16,
    pub rake_cap: u8,
    pub stage: HoldemStage,
    pub street: Street,
    pub street_bet: u64,
    pub board: Vec<String>,
    pub hand_index_map: BTreeMap<u64, Vec<usize>>,
    pub bet_map: BTreeMap<u64, u64>,
    pub total_bet_map: BTreeMap<u64, u64>,
    pub prize_map: BTreeMap<u64, u64>,
    pub player_map: BTreeMap<u64, Player>,
    pub player_order: Vec<u64>,
    pub pots: Vec<Pot>,
    pub acting_player: Option<ActingPlayer>,
    pub winners: Vec<u64>,
    pub display: Vec<Display>,
    pub mode: GameMode,
    pub table_size: u8, // The size of table
    pub hand_history: HandHistory,
    pub next_game_start: u64,
    pub rake_collected: u64,
}

// Methods that mutate or query the game state
impl Holdem {
    // calc timeout that should be wait after settle by state
    fn calc_pre_settle_timeout(&self) -> Result<u64, HandleError> {
        // 0.5s for collect chips, 5s for players observer game result
        let collet_chips_time = 500;
        let dealing_card_time = 1_500;
        let settle_pot_time = 4_000;
        let observe_result_time = 4_000;

        match self.stage {
            HoldemStage::Runner => {
                let timeout = collet_chips_time
                    + observe_result_time
                    + dealing_card_time * (self.board.len() as u64)
                    + settle_pot_time * (self.pots.len() as u64);
                Ok(timeout)
            }
            HoldemStage::Showdown => {
                let timeout = collet_chips_time
                    + observe_result_time
                    + settle_pot_time * (self.pots.len() as u64);
                Ok(timeout)
            }
            // for single player win, there's no need to observe game result
            HoldemStage::Settle => Ok(collet_chips_time + settle_pot_time),
            _ => Err(errors::wait_timeout_error_in_settle()),
        }
    }

    // After settle, remove players and checkpoint
    fn settle(&mut self, effect: &mut Effect) -> Result<(), HandleError> {
        // 1. remove players
        let removed_players = self.cash_table_kick_players(effect);
        for player in removed_players {
            effect.withdraw(player.id, player.chips + player.deposit);
            effect.eject(player.id);
        }
        // 2. transfer rake
        if self.rake_collected > 0 {
            effect.transfer(self.rake_collected);
        }
        // 3. do checkpoint
        effect.checkpoint();
        self.cash_table_reset_state()?;

        Ok(())
    }

    // Mark all eliminated players.
    // An eliminated player is one with zero chips.
    fn mark_eliminated_players(&mut self) {
        for p in self.player_map.values_mut() {
            if p.status != PlayerStatus::Leave && p.chips + p.deposit == 0 {
                p.status = PlayerStatus::Eliminated;
            }
        }
    }

    fn cash_table_kick_players(&mut self, effect: &mut Effect) -> Vec<Player> {
        if self.mode == GameMode::Cash {
            return self.kick_players(effect);
        }
        Vec::default()
    }

    fn cash_table_reset_state(&mut self) -> Result<(), HandleError> {
        if self.mode == GameMode::Cash {
            return self.reset_state();
        }
        Ok(())
    }

    // Remove players with `Leave`, `Out` or `Eliminated` status.
    pub fn kick_players(&mut self, effect: &mut Effect) -> Vec<Player> {
        let player_map = take(&mut self.player_map);
        let mut removed = Vec::new();
        let mut retained = Vec::new();

        for player in player_map.into_values() {
            if player.status == PlayerStatus::Leave {
                effect.info(format!("Remove player {} with Leave status", player.id));
                removed.push(player);
            } else if player.status == PlayerStatus::Out {
                effect.info(format!("Remove player {} with Out status", player.id));
                removed.push(player);
            } else if player.status == PlayerStatus::Eliminated {
                effect.info(format!(
                    "Remove player {} with Eliminated status",
                    player.id
                ));
                removed.push(player);
            } else {
                retained.push(player);
            }
        }

        self.player_map = retained.into_iter().map(|p| (p.id, p)).collect();
        println!("Remove these players: {:?}", removed);
        removed.into_iter().collect()
    }

    // Make All eligible players Wait
    pub fn reset_player_map_status(&mut self) -> Result<(), HandleError> {
        for player in self.player_map.values_mut() {
            if player.status == PlayerStatus::Out {
                player.timeout += 1;
            } else {
                player.status = PlayerStatus::Wait;
            }
        }
        Ok(())
    }

    // Clear data that don't belong to a running game, indicating game end
    // Additionally, cancel current dispatch
    fn signal_game_end(&mut self, effect: &mut Effect) -> Result<(), HandleError> {
        self.street_bet = 0;
        self.min_raise = 0;
        self.acting_player = None;
        effect.cancel_dispatch();
        Ok(())
    }

    /// Return the next acting player
    fn next_action_player(&mut self, next_players: Vec<u64>) -> Option<u64> {
        for id in next_players {
            if let Some(player) = self.player_map.get(&id) {
                let curr_bet: u64 = self.bet_map.get(&id).map(|b| *b).unwrap_or(0);
                if curr_bet < self.street_bet || player.status == PlayerStatus::Wait {
                    return Some(id);
                }
            }
        }
        None
    }

    pub fn is_acting_player(&self, player_id: u64) -> bool {
        match &self.acting_player {
            Some(ActingPlayer { id, .. }) => *id == player_id,
            None => false,
        }
    }

    fn get_remainder_player(&mut self) -> Option<u64> {
        let eligible_candidates = {
            let mut players = self
                .player_map
                .values()
                .filter(|p| {
                    self.prize_map.contains_key(&p.id())
                        && matches!(
                            p.status,
                            PlayerStatus::Acted | PlayerStatus::Allin | PlayerStatus::Wait
                        )
                })
                .map(|p| (p.id(), p.position))
                .collect::<Vec<(u64, usize)>>();
            players.sort_by(|(_, pos1), (_, pos2)| pos1.cmp(pos2));
            players.into_iter().map(|(id, _)| id).collect::<Vec<u64>>()
        };

        let remainder_player = if eligible_candidates.is_empty() {
            // When no remainder player, use the the first in player map
            self.player_map
                .first_key_value()
                .and_then(|(id, _)| Some(*id))
        } else {
            eligible_candidates.first().and_then(|id| Some(*id))
        };

        remainder_player
    }

    /// Return either acting player position or btn for reference
    fn get_ref_position(&self) -> usize {
        if let Some(ActingPlayer { position, .. }) = self.acting_player {
            position
        } else {
            self.btn
        }
    }

    // BTN moves clockwise.  The next BTN is calculated base on the current one
    pub fn get_next_btn(&mut self) -> Result<usize, HandleError> {
        let mut player_positions: Vec<usize> =
            self.player_map.values().map(|p| p.position).collect();
        player_positions.sort();

        let next_positions: Vec<usize> = player_positions
            .iter()
            .filter(|pos| **pos > self.btn)
            .map(|p| *p)
            .collect();

        if next_positions.is_empty() {
            let Some(next_btn) = player_positions.first() else {
                return Err(errors::next_button_player_not_found());
            };
            Ok(*next_btn)
        } else {
            if let Some(next_btn) = next_positions.first() {
                Ok(*next_btn)
            } else {
                return Err(errors::next_button_position_not_found());
            }
        }
    }

    fn get_action_time(&self) -> u64 {
        match self.street {
            Street::Turn => ACTION_TIMEOUT_TURN,
            Street::River => ACTION_TIMEOUT_RIVER,
            Street::Flop => ACTION_TIMEOUT_POSTFLOP,
            Street::Preflop => {
                if self.street_bet == self.bb {
                    ACTION_TIMEOUT_PREFLOP
                } else {
                    ACTION_TIMEOUT_POSTFLOP
                }
            }
            _ => 0,
        }
    }

    pub fn set_action_timeout(&mut self, effect: &mut Effect) -> Result<(), HandleError> {
        let Some(ref acting_player) = self.acting_player else {
            return Err(errors::internal_cannot_find_action_player())?;
        };
        if let Some(t) = acting_player.time_card_clock {
            effect.info("Set player action timeout according to time card timeout");
            effect.action_timeout(acting_player.id, t.saturating_sub(effect.timestamp()))?;
        } else {
            effect.info("Set player action timeout according to normal timeout");
            effect.action_timeout(
                acting_player.id,
                acting_player.clock.saturating_sub(effect.timestamp()),
            )?;
        }
        Ok(())
    }

    pub fn ask_for_action(
        &mut self,
        player_id: u64,
        effect: &mut Effect,
    ) -> Result<(), HandleError> {
        let mut timeout = self.get_action_time();

        if let Some(player) = self.player_map.get_mut(&player_id) {
            effect.info(format!("Asking {} to act", player.id));
            if player.is_afk {
                timeout = ACTION_TIMEOUT_AFK;
            }

            let action_start = effect.timestamp();
            let clock = action_start + timeout;
            if player.is_afk
                || player.time_cards == 0
                || (self.street == Street::Preflop && player.status == PlayerStatus::Wait)
            {
                self.acting_player = Some(ActingPlayer::new(
                    player.id,
                    player.position,
                    action_start,
                    clock,
                ));
            } else {
                self.acting_player = Some(ActingPlayer::new_with_time_card(
                    player.id,
                    player.position,
                    action_start,
                    clock,
                ));
            }
            player.status = PlayerStatus::Acting;
            self.set_action_timeout(effect)?;
            Ok(())
        } else {
            return Err(errors::next_action_player_missing());
        }
    }

    /// According to players position, place them in the following order:
    /// SB, BB, UTG (1st-to-act), MID (2nd-to-act), ..., BTN (last-to-act).
    pub fn arrange_players(&mut self, last_pos: usize) -> Result<(), HandleError> {
        let mut player_pos: Vec<(u64, usize)> = self
            .player_map
            .values()
            .filter(|p| p.status != PlayerStatus::Init)
            .map(|p| {
                if p.position > last_pos {
                    (p.id, p.position - last_pos)
                } else {
                    (p.id, p.position + 100)
                }
            })
            .collect();
        player_pos.sort_by(|(_, pos1), (_, pos2)| pos1.cmp(pos2));
        let player_order: Vec<u64> = player_pos.into_iter().map(|(id, _)| id).collect();
        println!("Player order {:?}", player_order);
        self.player_order = player_order;
        Ok(())
    }

    fn ante_bets(&mut self) -> Result<u64, HandleError> {
        if self.ante == 0 {
            return Ok(0);
        }

        let mut total_ante = 0;
        for player_id in self.player_order.clone() {
            let (allin, real_ante) = self.take_bet(player_id, self.ante)?;
            total_ante += real_ante;
            if allin {
                self.set_player_status(player_id, PlayerStatus::Allin)?;
            }
            self.hand_history
                .add_blinds_info(BlindBet::new(player_id, BlindType::Ante, real_ante));
        }

        Ok(total_ante)
    }

    pub fn blind_bets(&mut self, effect: &mut Effect) -> Result<(), HandleError> {
        let mut real_ante = 0;
        if self.ante > 0 {
            real_ante = self.ante_bets()?;
            self.collect_bets()?;
        }

        let (sb_id, bb_id) = if self.player_order.len() == 2 {
            let bb_id = self
                .player_order
                .first()
                .cloned()
                .ok_or(errors::heads_up_missing_sb())?;
            let sb_id = self
                .player_order
                .last()
                .cloned()
                .ok_or(errors::heads_up_missing_bb())?;
            (sb_id, bb_id)
        } else {
            let sb_id = self
                .player_order
                .get(0)
                .cloned()
                .ok_or(errors::mplayers_missing_sb())?;
            let bb_id = self
                .player_order
                .get(1)
                .cloned()
                .ok_or(errors::mplayers_missing_bb())?;
            (sb_id, bb_id)
        };

        let (allin, real_sb) = self.take_bet(sb_id, self.sb)?;
        if allin {
            self.set_player_status(sb_id, PlayerStatus::Allin)?;
        }
        let (allin, real_bb) = self.take_bet(bb_id, self.bb)?;
        if allin {
            self.set_player_status(bb_id, PlayerStatus::Allin)?;
        }

        let hh = &mut self.hand_history;
        hh.add_blinds_info(BlindBet::new(sb_id, BlindType::Sb, real_sb));
        hh.add_blinds_info(BlindBet::new(bb_id, BlindType::Bb, real_bb));
        hh.set_pot(Street::Preflop, real_sb + real_bb + real_ante);

        // Select next to act
        if self.player_order.len() == 2 {
            self.player_order.rotate_left(1);
        } else {
            self.player_order.rotate_left(2);
        }

        let mut action_addr = None;
        for addr in self.player_order.iter() {
            if let Some(player) = self.player_map.get_mut(addr) {
                if player.next_to_act() {
                    action_addr = Some(addr);
                    break;
                }
            }
        }

        match action_addr {
            Some(addr) => self.ask_for_action(addr.to_owned(), effect)?,
            None => self.next_state(effect)?, // players all go all in
        }

        self.min_raise = self.bb;
        self.street_bet = self.bb;
        self.display.push(Display::DealCards);
        Ok(())
    }

    /// Handle main pot and side pot(s), for example:
    /// Players A(100), B(45), C(45), D(50) call or go all in, then the pots become
    /// Main:  { amount: 45*4, owners: [A, B, C, D], winners [] }
    /// Side1: { amount: 5*2,  owners: [A, D], winners [] }
    /// Side2: { amount: 50,   owners: [A], winners [] } <-- should return bet to A
    pub fn collect_bets(&mut self) -> Result<(), HandleError> {
        let old_pots = self.pots.clone();
        // Remove any folded players from owners of a pot
        let unfolded_player_addrs: Vec<u64> = self
            .player_map
            .values()
            .filter(|p| {
                matches!(
                    p.status,
                    PlayerStatus::Wait
                        | PlayerStatus::Allin
                        | PlayerStatus::Acted
                        | PlayerStatus::Acting
                )
            })
            .map(|p| p.id)
            .collect();

        self.pots
            .iter_mut()
            .for_each(|p| p.owners.retain(|addr| unfolded_player_addrs.contains(addr)));

        // Filter bets: arrange from small to big and remove duplicates
        let mut bets: Vec<u64> = self.bet_map.iter().map(|(_, b)| *b).collect();
        bets.sort_by(|b1, b2| b1.cmp(b2));
        bets.dedup();
        println!(
            "In Street {:?} with these Bets: {:?}",
            self.street, self.bet_map
        );

        let mut new_pots = Vec::<Pot>::new();
        let mut acc: u64 = 0;
        for bet in bets {
            let mut owners: Vec<u64> = self
                .bet_map
                .iter()
                .filter(|(_, b)| **b >= bet)
                .map(|(owner, _)| owner.clone())
                .collect();
            let actual_bet = bet - acc;
            let amount = actual_bet * owners.len() as u64;
            // Pot with only 1 owner should return the bet in it to the owner
            if owners.len() == 1 {
                let owner = owners.first().ok_or(errors::internal_pot_has_no_owner())?;
                let receiver = self
                    .player_map
                    .get_mut(owner)
                    .ok_or(errors::internal_player_not_found())?;
                let total_bet = self
                    .total_bet_map
                    .get_mut(owner)
                    .ok_or(errors::internal_malformed_total_bet())?;
                receiver.chips += amount;
                *total_bet -= amount;
                continue;
            } else {
                owners.retain(|addr| unfolded_player_addrs.contains(addr));

                let mut pot = Pot::new(owners, amount);
                self.take_rake_from_pot(&mut pot)?;
                new_pots.push(pot);
                acc += actual_bet;
            }
        }

        // Merge pots with same (num of) owners
        for new_pot in new_pots.iter() {
            if let Some(last_pot) = self.pots.last_mut() {
                if new_pot.owners.len() == last_pot.owners.len() {
                    last_pot.merge(new_pot)?;
                } else {
                    self.pots.push(new_pot.clone());
                }
            } else {
                self.pots.push(new_pot.to_owned());
            }
        }

        println!("Pots after collecting bets: {:?}", self.pots);
        if !self.bet_map.is_empty() {
            self.display.push(Display::CollectBets {
                old_pots,
                bet_map: self.bet_map.clone(),
            });
        }
        self.bet_map.clear();
        Ok(())
    }

    pub fn change_street(
        &mut self,
        effect: &mut Effect,
        new_street: Street,
    ) -> Result<(), HandleError> {
        for player in self.player_map.values_mut() {
            if player.status == PlayerStatus::Acted {
                player.status = PlayerStatus::Wait;
            }
        }
        self.collect_bets()?;
        self.street = new_street;
        effect.info(format!("Street changes to {:?}", self.street));
        self.min_raise = self.bb;
        self.street_bet = 0;
        self.acting_player = None;
        self.update_board(effect)?;

        Ok(())
    }

    pub fn next_street(&mut self) -> Street {
        match self.street {
            Street::Init => Street::Preflop,
            Street::Preflop => Street::Flop,
            Street::Flop => Street::Turn,
            Street::Turn => Street::River,
            _ => Street::Showdown,
        }
    }

    /// Count players who haven't folded
    pub fn count_unfolded_players(&self) -> usize {
        self.player_map
            .values()
            .filter(|p| {
                matches!(
                    p.status,
                    PlayerStatus::Acted
                        | PlayerStatus::Allin
                        | PlayerStatus::Acting
                        | PlayerStatus::Wait
                )
            })
            .count()
    }

    /// Count players whose status is not `Init`
    pub fn count_ingame_players(&self) -> usize {
        self.player_map
            .values()
            .filter(|p| p.status != PlayerStatus::Init)
            .count()
    }

    /// Reveal community cards according to current street
    pub fn update_board(&mut self, effect: &mut Effect) -> Result<(), HandleError> {
        let players_cnt = self.count_ingame_players() * 2;
        match self.street {
            Street::Flop => {
                effect.reveal(
                    self.deck_random_id,
                    (players_cnt..(players_cnt + 3)).collect::<Vec<usize>>(),
                );
                self.stage = HoldemStage::ShareKey;
                effect.info(format!("Board is {:?}", self.board));
            }

            Street::Turn => {
                effect.reveal(self.deck_random_id, vec![players_cnt + 3]);
                self.stage = HoldemStage::ShareKey;
                effect.info(format!("Board is {:?}", self.board));
            }

            Street::River => {
                effect.reveal(self.deck_random_id, vec![players_cnt + 4]);
                self.stage = HoldemStage::ShareKey;
                effect.info(format!("Board is {:?}", self.board));
            }

            // For Runner, update 5 community cards at once
            Street::Showdown => {
                self.board.clear();
                let decryption = effect.get_revealed(self.deck_random_id)?;
                for i in players_cnt..(players_cnt + 5) {
                    if let Some(card) = decryption.get(&i) {
                        self.board.push(card.clone());
                    } else {
                        return Err(errors::internal_failed_to_reveal_board());
                    }
                }
                let board = self.board.clone();
                self.hand_history.set_board(board);
                effect.info(format!("Board is {:?}", self.board));
            }
            _ => {}
        }
        Ok(())
    }

    pub fn take_rake_from_pot(&mut self, pot: &mut Pot) -> Result<u64, HandleError> {
        if self.mode != GameMode::Cash {
            return Ok(0);
        }

        // No rake for preflop
        if self.street == Street::Preflop {
            return Ok(0);
        }

        let rake_to_take = u64::min(
            self.rake_cap as u64 * self.bb - self.rake_collected,
            pot.amount * self.rake as u64 / 1000,
        );

        pot.amount -= rake_to_take;
        self.rake_collected += rake_to_take;

        Ok(rake_to_take)
    }

    /// Build the prize map for awarding chips
    pub fn calc_prize(&mut self) -> Result<(), HandleError> {
        let pots = &mut self.pots;
        let mut prize_map = BTreeMap::<u64, u64>::new();
        // TODO: discuss the smallest unit
        let smallest_bet = 1u64;
        let mut odd_chips = 0u64;
        for pot in pots {
            let cnt: u64 = pot.winners.len() as u64;
            let remainder = pot.amount % (smallest_bet * cnt);
            odd_chips += remainder;
            let prize: u64 = (pot.amount - remainder) / cnt;
            println!("Pot amount = {}", pot.amount);
            println!("Pot winner number = {}", cnt);
            println!("Pot remainder = {}", remainder);
            println!("Pot prize = {}", prize);
            for winner in pot.winners.iter() {
                prize_map
                    .entry(*winner)
                    .and_modify(|p| *p += prize)
                    .or_insert(prize);
            }
        }

        // Giving odd chips to remainder player
        let remainder_player = self
            .get_remainder_player()
            .ok_or(errors::internal_player_not_found())?;

        println!(
            "Player {} to get the {} odd chips",
            remainder_player, odd_chips
        );
        prize_map
            .entry(remainder_player)
            .and_modify(|prize| *prize += odd_chips)
            .or_insert(odd_chips);

        self.prize_map = prize_map;
        Ok(())
    }

    /// Increase player chips according to prize map.
    /// Chips of those who lost will be left untouched as
    /// their chips will be updated later by update_chips_map.
    pub fn apply_prize(&mut self) -> Result<(), HandleError> {
        for player in self.player_map.values_mut() {
            match self.prize_map.get(&player.id) {
                Some(prize) => {
                    player.chips += *prize;
                    println!("Player {} won {} chips", player.id, *prize);
                }
                None => {
                    println!("Player {} lost the bet", player.id);
                }
            }
        }
        Ok(())
    }

    /// winner_sets:
    /// examples: [[alice, bob], [charlie, dave]] can be used to represent Royal flush: alice, bob > Flush: charlie, dave
    /// Return award pots.
    pub fn assign_winners(
        &mut self,
        winner_sets: Vec<Vec<u64>>,
    ) -> Result<Vec<AwardPot>, HandleError> {
        for pot in self.pots.iter_mut() {
            for winner_set in winner_sets.iter() {
                let real_winners: Vec<u64> = winner_set
                    .iter()
                    .filter(|w| pot.owners.contains(*w))
                    .map(|w| *w)
                    .collect();
                // A pot should have at least one winner
                if real_winners.len() >= 1 {
                    pot.winners = real_winners;
                    break;
                }
            }
            if pot.winners.is_empty() {
                return Err(errors::pot_winner_missing());
            }
        }

        let award_pots = self
            .pots
            .iter()
            .map(|pot| {
                let winners = pot.winners.clone();
                let amount = pot.amount;
                AwardPot { winners, amount }
            })
            .collect();

        Ok(award_pots)
    }

    fn create_game_result_display(
        &mut self,
        award_pots: Vec<AwardPot>,
        player_result_map: BTreeMap<u64, PlayerResult>,
    ) {
        self.display.push(Display::GameResult {
            award_pots,
            player_map: player_result_map,
        });
    }

    /// Update the map that records players chips change (increased or decreased)
    /// Used for settlement
    pub fn update_player_chips(&mut self) -> Result<BTreeMap<u64, PlayerResult>, HandleError> {
        // The i64 change for each player.  The amount = total pots
        // earned - total bet.  This map will be returned for furture
        // calculation.
        let mut chips_change_map: BTreeMap<u64, i64> =
            self.player_map.keys().map(|id| (*id, 0)).collect();

        // The players for game result information.  The `chips` is
        // the amount before the settlement, the `prize` is the sum of
        // pots earned during the settlement.  This map will be added
        // to display.
        let mut player_result_map = BTreeMap::<u64, PlayerResult>::new();

        self.winners = Vec::<u64>::with_capacity(self.player_map.len());

        println!("Chips map before awarding: {:?}", chips_change_map);
        println!("Totol bet map: {:?}", self.total_bet_map);

        for (player, total_bet) in self.total_bet_map.iter() {
            chips_change_map
                .entry(player.clone())
                .and_modify(|chg| *chg -= *total_bet as i64);
        }

        for (player, prize) in self.prize_map.iter() {
            if *prize > 0 {
                self.winners.push(player.clone());
            }
            chips_change_map
                .entry(player.clone())
                .and_modify(|chips| *chips += *prize as i64);
        }

        println!("Chips map after awarding: {:?}", chips_change_map);

        for (id, player) in self.player_map.iter() {
            let result = PlayerResult {
                id: *id,
                position: player.position,
                status: player.status,
                chips: player.chips,
            };

            player_result_map.insert(*id, result);
        }

        self.hand_history.set_chips_change(&chips_change_map);
        Ok(player_result_map)
    }

    pub fn single_player_win(
        &mut self,
        effect: &mut Effect,
        winner: u64,
    ) -> Result<(), HandleError> {
        self.collect_bets()?;
        let award_pots = self.assign_winners(vec![vec![winner]])?;
        self.calc_prize()?;
        let player_result_map = self.update_player_chips()?;
        self.create_game_result_display(award_pots, player_result_map);
        self.apply_prize()?;
        self.mark_eliminated_players();
        self.hand_history.valid = true;
        // after settle, waiting timeout, for animations
        let timeout = self.calc_pre_settle_timeout()?;
        self.wait_timeout(effect, timeout);

        Ok(())
    }

    pub fn wait_timeout(&mut self, effect: &mut Effect, timeout: u64) {
        self.next_game_start = effect.timestamp() + timeout;
        effect.wait_timeout(timeout);
    }

    pub fn fill_player_chips_with_deposits(&mut self) {
        for player in self.player_map.values_mut() {
            if player.chips < self.max_deposit && player.deposit > 0 {
                let old_player_chips = player.chips;
                player.chips = u64::min(player.chips + player.deposit, self.max_deposit);
                player.deposit = player.deposit - player.chips + old_player_chips;
            }
        }
    }

    pub fn update_game_result(&mut self, effect: &mut Effect) -> Result<(), HandleError> {
        let decryption = effect.get_revealed(self.deck_random_id)?;
        // Board
        let board: Vec<&str> = self.board.iter().map(|c| c.as_str()).collect();
        // Player hands
        let mut player_hands: Vec<(u64, PlayerHand)> = Vec::with_capacity(self.player_order.len());

        let mut showdowns = Vec::<(u64, Showdown)>::new();

        for (id, idxs) in self.hand_index_map.iter() {
            if idxs.len() != 2 {
                return Err(errors::invalid_hole_cards_number());
            }

            let Some(player) = self.player_map.get(id) else {
                return Err(errors::internal_player_not_found());
            };

            if player.status != PlayerStatus::Fold
                && player.status != PlayerStatus::Init
                && player.status != PlayerStatus::Leave
            {
                let Some(first_card_idx) = idxs.first() else {
                    return Err(errors::first_hole_card_index_missing());
                };
                let Some(first_card) = decryption.get(first_card_idx) else {
                    return Err(errors::first_hole_card_error());
                };
                let Some(second_card_idx) = idxs.last() else {
                    return Err(errors::second_hole_card_index_missing());
                };
                let Some(second_card) = decryption.get(second_card_idx) else {
                    return Err(errors::second_hole_card_error());
                };
                let hole_cards = [first_card.as_str(), second_card.as_str()];
                let cards = create_cards(board.as_slice(), &hole_cards);
                let hand = evaluate_cards(cards);
                let hole_cards = hole_cards.iter().map(|c| c.to_string()).collect();
                let category = hand.category.clone();
                let picks = hand.picks.iter().map(|c| c.to_string()).collect();
                player_hands.push((*id, hand));
                showdowns.push((
                    *id,
                    Showdown {
                        hole_cards,
                        category,
                        picks,
                    },
                ));
            }
        }
        player_hands.sort_by(|(_, h1), (_, h2)| compare_hands(&h2.value, &h1.value));

        println!("Player Hands from strong to weak {:?}", player_hands);

        // Winners example: [[w1], [w2, w3], ... ] where w2 == w3, i.e. a draw/tie
        let mut winners: Vec<Vec<u64>> = Vec::new();
        let mut weaker: Vec<Vec<u64>> = Vec::new();
        // Players in a draw will be in the same set
        let mut draws = Vec::<u64>::new();
        // Each hand is either equal to or weaker than winner (1st)
        let Some((winner, highest_hand)) = player_hands.first() else {
            return Err(errors::strongest_hand_not_found());
        };

        for (player, hand) in player_hands.iter().skip(1) {
            if highest_hand.value.iter().eq(hand.value.iter()) {
                draws.push(player.clone());
            } else {
                weaker.push(vec![player.clone()]);
            }
        }

        if draws.len() > 0 {
            draws.push(winner.clone());
            winners.extend_from_slice(&vec![draws]);
        } else {
            winners.push(vec![winner.clone()]);
        }

        if weaker.len() > 0 {
            winners.extend_from_slice(&weaker);
        }

        println!("Player rankings in order: {:?}", winners);

        let award_pots = self.assign_winners(winners)?;
        self.calc_prize()?;

        let player_result_map = self.update_player_chips()?;
        self.create_game_result_display(award_pots, player_result_map);
        self.apply_prize()?;
        self.hand_history.valid = true;
        self.mark_eliminated_players();

        // Save to hand history
        for (id, showdown) in showdowns.into_iter() {
            self.hand_history.add_showdown(id, showdown);
        }

        Ok(())
    }

    // De facto entry point of Holdem
    pub fn next_state(&mut self, effect: &mut Effect) -> Result<(), HandleError> {
        let last_pos = self.get_ref_position();
        self.arrange_players(last_pos)?;
        // ingame_players exclude anyone with `Init` status
        let ingame_players = self.player_order.clone();
        let mut players_to_stay = Vec::<u64>::new();
        let mut players_to_act = Vec::<u64>::new();
        let mut players_allin = Vec::<u64>::new();

        for id in ingame_players.iter() {
            if let Some(player) = self.player_map.get(id) {
                match player.status {
                    PlayerStatus::Acting => {
                        players_to_stay.push(*id);
                    }
                    PlayerStatus::Wait | PlayerStatus::Acted => {
                        players_to_stay.push(*id);
                        players_to_act.push(*id);
                    }
                    PlayerStatus::Allin => {
                        players_to_stay.push(*id);
                        players_allin.push(*id);
                    }
                    _ => {}
                }
            }
        }

        let next_player = self.next_action_player(players_to_act);
        let next_street = self.next_street();

        // Single player wins because there is one player only
        // It happends when the last second player left the game
        if ingame_players.len() == 1 {
            self.stage = HoldemStage::Settle;
            self.signal_game_end(effect)?;
            let Some(winner) = ingame_players.first() else {
                return Err(errors::single_player_missing());
            };
            println!("[Next State]: Single winner: {}", winner);
            self.single_player_win(effect, winner.clone())?;
            Ok(())
        }
        // Single players wins because others all folded
        else if players_to_stay.len() == 1 {
            self.stage = HoldemStage::Settle;
            self.signal_game_end(effect)?;
            let Some(winner) = players_to_stay.first() else {
                return Err(errors::single_winner_missing());
            };
            println!(
                "[Next State]: All others folded and single winner is {}",
                winner
            );
            self.single_player_win(effect, (*winner).clone())?;
            Ok(())
        }
        // Blind bets
        else if self.street == Street::Preflop && self.bet_map.is_empty() {
            println!("[Next State]: Blind bets");
            self.blind_bets(effect)?;
            Ok(())
        }
        // Ask next player to act
        else if next_player.is_some() {
            let Some(next_action_player) = next_player else {
                return Err(errors::next_action_player_missing());
            };
            println!(
                "[Next State]: Next-to-act player is: {}",
                next_action_player
            );
            self.ask_for_action(next_action_player, effect)?;
            Ok(())
        }
        // Runner
        else if self.stage != HoldemStage::Runner
            && players_allin.len() + 1 >= players_to_stay.len()
        {
            println!("[Next State]: Runner");
            self.street = Street::Showdown;
            self.stage = HoldemStage::Runner;
            self.signal_game_end(effect)?;
            self.collect_bets()?;

            // Reveal all cards for eligible players: not folded and without init status
            for (id, idxs) in self.hand_index_map.iter() {
                let Some(player) = self.player_map.get(id) else {
                    return Err(errors::internal_player_not_in_game_but_assigned_cards());
                };
                if matches!(player.status, PlayerStatus::Acted | PlayerStatus::Allin) {
                    effect.reveal(self.deck_random_id, idxs.clone());
                }
            }

            let board_start = self.hand_index_map.len() * 2;
            effect.reveal(
                self.deck_random_id,
                (board_start..(board_start + 5)).collect(),
            );
            Ok(())
        }
        // Next Street
        else if next_street != Street::Showdown {
            println!("[Next State]: Move to next street: {:?}", next_street);
            self.change_street(effect, next_street)?;
            let street = self.street;
            let total_pot = self.pots.iter().map(|p| p.amount).sum();
            self.hand_history.set_pot(street, total_pot);
            Ok(())
        }
        // Showdown
        else {
            println!("[Next State]: Showdown");
            self.stage = HoldemStage::Showdown;
            self.street = Street::Showdown;
            self.signal_game_end(effect)?;
            self.collect_bets()?;

            // Reveal players' hole cards
            for (addr, idxs) in self.hand_index_map.iter() {
                let Some(player) = self.player_map.get(addr) else {
                    return Err(errors::internal_player_not_in_game_but_assigned_cards());
                };
                if matches!(player.status, PlayerStatus::Acted | PlayerStatus::Allin) {
                    effect.reveal(self.deck_random_id, idxs.clone());
                }
            }

            Ok(())
        }
    }

    pub fn handle_custom_event(
        &mut self,
        effect: &mut Effect,
        event: GameEvent,
        sender: u64,
    ) -> Result<(), HandleError> {
        match event {
            GameEvent::Bet(_)
            | GameEvent::Check
            | GameEvent::Fold
            | GameEvent::Raise(_)
            | GameEvent::Call => {
                return self.handle_action_event(effect, event, sender);
            }

            GameEvent::SitOut => {
                self.set_player_status(sender, PlayerStatus::Leave)?;
                let _ = self.handle_player_leave(effect, sender);
                Ok(())
            }

            GameEvent::UseTimeCard => {
                let Some(ref mut acting_player) = self.acting_player else {
                    return Err(errors::not_the_acting_player())?;
                };
                if acting_player.id != sender {
                    return Err(errors::not_the_acting_player())?;
                }
                let Some(ref player) = self.player_map.get(&sender) else {
                    return Err(HandleError::InvalidPlayer);
                };
                if player.time_cards == 0 {
                    return Err(errors::no_time_cards())?;
                }
                if acting_player.time_card_clock.is_some() {
                    return Err(errors::time_card_already_in_use())?;
                }
                acting_player.time_card_clock =
                    Some(acting_player.clock + TIME_CARD_EXTRA_SECS * 1000);
                self.set_action_timeout(effect)?;
                Ok(())
            }

            GameEvent::SitIn => {
                let Some(player) = self.player_map.get_mut(&sender) else {
                    return Err(HandleError::InvalidPlayer);
                };
                player.is_afk = false;

                Ok(())
            }
        }
    }

    pub fn reduce_time_cards(&mut self, effect: &mut Effect) -> Result<(), HandleError> {
        let Some(ref acting_player) = self.acting_player else {
            return Err(errors::internal_cannot_find_action_player())?;
        };
        // When the player used at least one time card
        if acting_player.time_card_clock.is_some() && effect.timestamp() > acting_player.clock {
            let Some(player) = self.player_map.get_mut(&acting_player.id) else {
                return Err(errors::internal_player_not_found())?;
            };
            player.time_cards -= 1;
        }
        Ok(())
    }

    pub fn handle_action_event(
        &mut self,
        effect: &mut Effect,
        event: GameEvent,
        sender: u64,
    ) -> Result<(), HandleError> {
        let Some(player) = self.player_map.get(&sender) else {
            return Err(HandleError::InvalidPlayer);
        };

        let player_id = player.id;

        match event {
            GameEvent::Bet(amount) => {
                if !self.is_acting_player(sender) {
                    return Err(errors::not_the_acting_player_to_bet());
                }
                if self.bet_map.get(&sender).is_some() {
                    return Err(errors::player_already_betted());
                }
                // Freestyle betting not allowed in the preflop
                if self.street_bet != 0 {
                    return Err(errors::player_cant_bet());
                }

                // When bet amount is less than 1BB, only allin is allowed.
                if self.bb > amount && player.chips != amount {
                    return Err(errors::bet_amonut_is_too_small());
                }

                let (allin, real_bet_amount) = self.take_bet(sender.clone(), amount)?;
                self.set_player_acted(sender, allin)?;
                self.min_raise = amount;
                self.street_bet = amount;
                self.hand_history.add_action(self.street, PlayerAction::new_bet(player_id, real_bet_amount))?;
                self.reduce_time_cards(effect)?;
            }

            GameEvent::Call => {
                if !self.is_acting_player(sender) {
                    return Err(errors::not_the_acting_player_to_call());
                }

                let betted = self.get_player_bet(sender);
                let call_amount = self.street_bet - betted;
                let (allin, real_call_amount) = self.take_bet(sender.clone(), call_amount)?;
                self.set_player_acted(sender, allin)?;
                self.hand_history.add_action(self.street, PlayerAction::new_call(player_id, real_call_amount))?;
                self.reduce_time_cards(effect)?;
            }

            GameEvent::Check => {
                if !self.is_acting_player(sender) {
                    return Err(errors::not_the_acting_player_to_check());
                }

                // Check is only available when player's current bet equals street bet.
                let curr_bet = self.get_player_bet(sender);
                if curr_bet != self.street_bet {
                    return Err(errors::player_cant_check());
                }
                self.set_player_status(sender, PlayerStatus::Acted)?;
                self.hand_history.add_action(self.street, PlayerAction::new_check(player_id))?;
                self.reduce_time_cards(effect)?;
            }

            GameEvent::Fold => {
                if !self.is_acting_player(sender) {
                    return Err(errors::not_the_acting_player_to_fold());
                }
                self.set_player_status(sender, PlayerStatus::Fold)?;
                self.hand_history.add_action(self.street, PlayerAction::new_fold(player_id))?;
                self.reduce_time_cards(effect)?;
            }

            GameEvent::Raise(amount) => {
                if !self.is_acting_player(sender) {
                    return Err(errors::not_the_acting_player_to_raise());
                }

                if self.street_bet == 0 || self.bet_map.is_empty() {
                    return Err(errors::player_cant_raise());
                }

                let betted = self.get_player_bet(sender);
                if amount + betted < self.street_bet + self.min_raise && amount != player.chips {
                    return Err(errors::raise_amount_is_too_small());
                }
                let (allin, real_raise_amount) = self.take_bet(sender.clone(), amount)?;
                self.set_player_acted(sender, allin)?;
                let new_street_bet = betted + real_raise_amount;
                let new_min_raise = new_street_bet - self.street_bet;
                self.street_bet = new_street_bet;
                self.min_raise = new_min_raise;
                self.hand_history.add_action(self.street, PlayerAction::new_raise(player_id, betted + real_raise_amount))?;
                self.reduce_time_cards(effect)?;
            }

            _ => {}
        }

        self.next_state(effect)?;
        Ok(())
    }

    fn handle_player_leave(
        &mut self,
        effect: &mut Effect,
        player_id: u64,
    ) -> Result<(), HandleError> {
        match self.stage {
            // If current stage is not playing, the player can
            // leave with a settlement instantly.
            HoldemStage::Init | HoldemStage::Settle => {
                if let Some(player) = self.player_map.get_mut(&player_id) {
                    effect.info(format!(
                        "Player {} leaves game, current stage: {:?}",
                        player_id, self.stage
                    ));
                    effect.checkpoint();
                    player.status = PlayerStatus::Leave;
                    self.wait_timeout(effect, WAIT_TIMEOUT_DEFAULT);
                    self.signal_game_end(effect)?;
                    // Eject all left players
                    for p in self.cash_table_kick_players(effect) {
                        effect.withdraw(p.id, p.chips + p.deposit);
                        effect.eject(p.id);
                    }
                } else {
                    return Err(HandleError::InvalidPlayer)?;
                }
            }

            // If current stage is waiting for a settlement.
            // Mark the leaving player as `Leave`.
            // Then wait for settlement.
            HoldemStage::Runner | HoldemStage::Showdown => {
                if let Some(player) = self.player_map.get_mut(&player_id) {
                    effect.info(format!(
                        "Player {} leaves game, current stage: {:?}",
                        player_id, self.stage
                    ));
                    player.status = PlayerStatus::Leave;
                    effect.checkpoint();
                } else {
                    return Err(HandleError::InvalidPlayer)?;
                }
            }

            // If current stage is playing, the player will be
            // marked as `Leave`.  There are 3 cases to
            // handle:
            //
            // 1. The leaving player is the
            // second last player, so the remaining player
            // just wins.
            //
            // 2. The leaving player is in acting.  In such
            // case, we just fold this player and do next
            // state calculation.
            //
            // 3. The leaving player is not the acting player,
            // and the game can continue.
            HoldemStage::Play | HoldemStage::ShareKey => {
                let unfolded_cnt = self.count_unfolded_players();
                if self.stage == HoldemStage::Play
                    && !self.is_acting_player(player_id)
                    && unfolded_cnt > 1
                {
                    effect.info(format!(
                        "Player {} leaves before its turn, game continues",
                        player_id
                    ));
                } else if self.is_acting_player(player_id) {
                    effect.info(format!(
                        "Player {} folds and leaves, game continues",
                        player_id
                    ));
                    self.next_state(effect)?;
                } else if unfolded_cnt == 1 {
                    effect.info(format!("Player {} leaves, game settles", player_id));
                    let winner = self
                        .player_map
                        .values()
                        .find(|p| p.id() != player_id)
                        .map(|p| p.id())
                        .ok_or(errors::single_winner_missing())?;
                    self.stage = HoldemStage::Settle;
                    self.single_player_win(effect, winner)?;
                    self.signal_game_end(effect)?;
                }
            }
        }

        Ok(())
    }

    pub fn set_player_acted(&mut self, player_id: u64, allin: bool) -> Result<(), HandleError> {
        self.set_player_status(
            player_id,
            if allin {
                PlayerStatus::Allin
            } else {
                PlayerStatus::Acted
            },
        )
    }

    pub fn reset_player_timeout(&mut self, player_id: u64) -> Result<(), HandleError> {
        let Some(player) = self.player_map.get_mut(&player_id) else {
            return Err(HandleError::InvalidPlayer);
        };
        player.timeout = 0;
        Ok(())
    }

    pub fn set_player_status(
        &mut self,
        player_id: u64,
        status: PlayerStatus,
    ) -> Result<(), HandleError> {
        let Some(player) = self.player_map.get_mut(&player_id) else {
            return Err(HandleError::InvalidPlayer);
        };
        player.status = status;
        Ok(())
    }

    pub fn get_player_bet(&self, player_id: u64) -> u64 {
        self.bet_map.get(&player_id).cloned().unwrap_or(0)
    }

    pub fn take_bet(&mut self, player_id: u64, amount: u64) -> Result<(bool, u64), HandleError> {
        let Some(player) = self.player_map.get_mut(&player_id) else {
            return Err(HandleError::InvalidPlayer);
        };
        let (allin, real_amount) = player.take_bet(amount);
        self.bet_map
            .entry(player_id)
            .and_modify(|amt| *amt += real_amount)
            .or_insert(real_amount);
        self.total_bet_map
            .entry(player_id)
            .and_modify(|amt| *amt += real_amount)
            .or_insert(real_amount);
        Ok((allin, real_amount))
    }

    pub fn reset_state(&mut self) -> Result<(), HandleError> {
        self.deck_random_id = 0;
        self.min_raise = 0;
        self.stage = HoldemStage::Init;
        self.street = Street::Init;
        self.street_bet = 0;
        self.board.clear();
        self.hand_index_map.clear();
        self.bet_map.clear();
        self.total_bet_map.clear();
        self.prize_map.clear();
        self.player_order.clear();
        self.pots.clear();
        self.acting_player = None;
        self.winners.clear();
        self.display.clear();
        self.hand_history = HandHistory::default();
        self.next_game_start = 0;
        self.rake_collected = 0;
        // Reset player status
        self.reset_player_map_status()?;
        Ok(())
    }

    pub fn find_position(&self) -> Option<u8> {
        for i in 0..self.table_size {
            if self
                .player_map
                .iter()
                .find(|p| p.1.position == i as usize)
                .is_none()
            {
                return Some(i);
            }
        }
        return None;
    }

    pub fn position_occupied(&self, position: usize) -> bool {
        self.player_map
            .iter()
            .find(|(_, ref p)| p.position == position)
            .is_some()
    }

    pub fn internal_add_players(
        &mut self,
        add_players: Vec<InternalPlayerJoin>,
    ) -> Result<(), HandleError> {
        for p in add_players {
            // Since it's an internal event, we have to take care of
            // position.
            let occupied_pos = self
                .player_map
                .values()
                .map(|p| p.position)
                .collect::<Vec<usize>>();
            let Some(pos) = (0..11).find(|i| !occupied_pos.contains(i)) else {
                return Err(errors::cannot_join_full_table());
            };

            self.player_map.insert(
                p.id,
                Player::new_with_defaults(p.id, p.chips, pos, PlayerStatus::Fold),
            );
        }
        Ok(())
    }

    pub fn internal_start_game(&mut self, effect: &mut Effect) -> Result<(), HandleError> {
        self.reset_state()?;
        self.fill_player_chips_with_deposits();

        let next_btn = self.get_next_btn()?;
        effect.info(format!("Game starts and next BTN: {}", next_btn));
        self.btn = next_btn;

        // Only start game when there are at least two available player
        if self
            .player_map
            .iter()
            .filter(|p| !matches!(p.1.status, PlayerStatus::Out | PlayerStatus::Leave))
            .count()
            >= 2
        {
            // Prepare randomness (shuffling cards)
            let rnd_spec = RandomSpec::deck_of_cards();
            self.deck_random_id = effect.init_random_state(rnd_spec);
        }
        self.hand_id += 1;

        Ok(())
    }
}

impl GameHandler for Holdem {
    fn balances(&self) -> Vec<PlayerBalance> {
        self.player_map
            .values()
            .map(|p| PlayerBalance::new(p.id, p.chips + p.deposit))
            .collect()
    }

    fn init_state(init_account: InitAccount) -> Result<Self, HandleError> {
        let HoldemAccount {
            sb,
            bb,
            ante,
            max_deposit,
            rake,
            rake_cap,
            ..
        } = init_account.data()?;

        let btn = 0;
        let next_game_start = 0;

        Ok(Self {
            deck_random_id: 0,
            max_deposit,
            sb,
            bb,
            ante,
            min_raise: bb,
            btn,
            rake,
            rake_cap,
            stage: HoldemStage::Init,
            street: Street::Init,
            street_bet: 0,
            next_game_start,
            mode: GameMode::Cash,
            table_size: init_account.max_players as _,
            ..Default::default()
        })
    }

    fn handle_event(&mut self, effect: &mut Effect, event: Event) -> Result<(), HandleError> {
        self.display.clear();
        match event {
            // Handle holdem specific (custom) events
            Event::Custom { sender, raw } => {
                self.reset_player_timeout(sender)?;
                let event: GameEvent = GameEvent::try_parse(&raw)?;

                effect.info(format!(
                    "Player action event: {:?}, sender: {:?}",
                    event, sender
                ));

                self.handle_custom_event(effect, event, sender)?;

                Ok(())
            }

            Event::ActionTimeout { player_id } => {
                if !self.is_acting_player(player_id) {
                    return Err(errors::not_the_acting_player());
                }

                self.reduce_time_cards(effect)?;

                let Some(player) = self.player_map.get_mut(&player_id) else {
                    return Err(errors::internal_player_not_found());
                };
                player.timeout += 1;

                let street = self.street;
                // In Cash game, mark those who've reached T/O for
                // MAX_ACTION_TIMEOUT_COUNT times with `Leave` status
                if player.timeout >= MAX_ACTION_TIMEOUT_COUNT {
                    if self.mode == GameMode::Cash {
                        self.set_player_status(player_id, PlayerStatus::Leave)?;
                        self.hand_history.add_action(street, PlayerAction::new_fold(player_id))?;
                        self.next_state(effect)?;
                        return Ok(());
                    } else {
                        player.is_afk = true;
                    }
                }

                if self.mode == GameMode::Cash {
                    if player.timeout >= MAX_ACTION_TIMEOUT_COUNT {
                        self.set_player_status(player_id, PlayerStatus::Leave)?;
                        self.hand_history.add_action(street, PlayerAction::new_fold(player_id))?;
                        self.next_state(effect)?;
                        return Ok(());
                    } else if street != Street::Preflop {
                        player.timeout += 1;
                    }
                }

                let street_bet = self.street_bet;
                let bet = if let Some(player_bet) = self.bet_map.get(&player_id) {
                    *player_bet
                } else {
                    0
                };

                if bet == street_bet {
                    self.set_player_status(player_id, PlayerStatus::Acted)?;
                    self.hand_history.add_action(street, PlayerAction::new_check(player_id))?;
                    self.next_state(effect)?;
                    Ok(())
                } else {
                    self.set_player_status(player_id, PlayerStatus::Fold)?;
                    self.hand_history.add_action(street, PlayerAction::new_fold(player_id))?;
                    self.next_state(effect)?;
                    Ok(())
                }
            }

            Event::WaitingTimeout => {
                self.settle(effect)?;
                if self.player_map.len() >= 2
                    && effect.count_nodes() >= 1
                    && self.mode != GameMode::Mtt
                {
                    effect.start_game();
                }
                Ok(())
            }

            Event::Ready => {
                if self.player_map.len() >= 2 && effect.count_nodes() >= 1 {
                    effect.start_game();
                }
                Ok(())
            }

            Event::Join { players } => {
                effect.info("A player joined!");

                for p in players.into_iter() {
                    let player = Player::init(p.id(), 0, p.position());
                    self.player_map.insert(p.id(), player);
                }

                match self.stage {
                    HoldemStage::Init => {
                        if self.player_map.len() >= 2 && effect.count_nodes() >= 1 {
                            effect.start_game();
                        }
                    }

                    HoldemStage::Runner | HoldemStage::Settle | HoldemStage::Showdown => {
                        if self.next_game_start > effect.timestamp() {
                            effect.wait_timeout(self.next_game_start - effect.timestamp());
                        } else if self.player_map.len() >= 2 && effect.count_nodes() >= 1 {
                            effect.start_game();
                        }
                    }

                    _ => (),
                }

                Ok(())
            }

            Event::Deposit { deposits } => {
                for d in deposits.into_iter() {
                    if let Some(p) = self.player_map.get_mut(&d.id()) {
                        if p.chips + p.deposit > 2 * self.max_deposit {
                            effect.reject_deposit(&d)?;
                        } else {
                            p.deposit += d.balance();
                            effect.accept_deposit(&d)?;
                        }
                    } else {
                        effect.reject_deposit(&d)?;
                    }
                }
                Ok(())
            }

            Event::GameStart => {
                self.internal_start_game(effect)?;
                Ok(())
            }

            Event::Leave { player_id } => {
                // TODO: Leaving is not allowed in SNG game
                self.set_player_status(player_id, PlayerStatus::Leave)?;
                let _ = self.handle_player_leave(effect, player_id);

                Ok(())
            }

            Event::RandomnessReady { .. } => {
                // Cards are dealt to players but remain invisible to them
                for (idx, (id, player)) in self.player_map.iter().enumerate() {
                    if matches!(player.status, PlayerStatus::Wait) {
                        effect.assign(self.deck_random_id, *id, vec![idx * 2, idx * 2 + 1])?;
                        self.hand_index_map.insert(*id, vec![idx * 2, idx * 2 + 1]);
                    }
                }

                Ok(())
            }

            Event::SecretsReady { .. } => match self.stage {
                HoldemStage::ShareKey => {
                    let players_cnt = self.count_ingame_players() * 2;
                    let board_prev_cnt = self.board.len();
                    self.stage = HoldemStage::Play;

                    match self.street {
                        Street::Preflop => {
                            self.next_state(effect)?;
                        }

                        Street::Flop => {
                            let decryption = effect.get_revealed(self.deck_random_id)?;
                            for i in players_cnt..(players_cnt + 3) {
                                if let Some(card) = decryption.get(&i) {
                                    self.board.push(card.clone());
                                } else {
                                    return Err(errors::flop_cards_error());
                                }
                            }
                            self.display.push(Display::DealBoard {
                                prev: board_prev_cnt,
                                board: self.board.clone(),
                            });
                            self.hand_history.set_board(self.board.clone());
                            self.next_state(effect)?;
                        }

                        Street::Turn => {
                            let decryption = effect.get_revealed(self.deck_random_id)?;
                            let card_index = players_cnt + 3;
                            if let Some(card) = decryption.get(&card_index) {
                                self.board.push(card.clone());
                                self.display.push(Display::DealBoard {
                                    prev: board_prev_cnt,
                                    board: self.board.clone(),
                                });
                            } else {
                                return Err(errors::turn_card_error());
                            }

                            self.hand_history.set_board(self.board.clone());
                            self.next_state(effect)?;
                        }

                        Street::River => {
                            let decryption = effect.get_revealed(self.deck_random_id)?;
                            let card_index = players_cnt + 4;
                            if let Some(card) = decryption.get(&card_index) {
                                self.board.push(card.clone());
                                if board_prev_cnt != 5 {
                                    self.display.push(Display::DealBoard {
                                        prev: board_prev_cnt,
                                        board: self.board.clone(),
                                    });
                                }
                            } else {
                                return Err(errors::river_card_error());
                            }

                            self.hand_history.set_board(self.board.clone());
                            self.next_state(effect)?;
                        }

                        _ => {}
                    }
                    Ok(())
                }

                // Shuffling deck
                HoldemStage::Init => {
                    match self.street {
                        Street::Init => {
                            self.street = Street::Preflop;
                            self.stage = HoldemStage::Play;
                            self.next_state(effect)?;
                            Ok(())
                        }

                        // if other streets, keep playing
                        _ => Ok(()),
                    }
                }

                // Ending, comparing cards
                HoldemStage::Runner => {
                    let prev_board_cnt = self.board.len();
                    self.update_board(effect)?;
                    if prev_board_cnt != 5 {
                        self.display.push(Display::DealBoard {
                            prev: prev_board_cnt,
                            board: self.board.clone(),
                        });
                    }

                    self.update_game_result(effect)?;
                    let timeout = self.calc_pre_settle_timeout()?;
                    // WAIT_TIMEOUT_RUNNER
                    self.wait_timeout(effect, timeout);

                    Ok(())
                }

                // Ending, comparing cards
                HoldemStage::Showdown => {
                    self.update_game_result(effect)?;
                    let timeout = self.calc_pre_settle_timeout()?;
                    // WAIT_TIMEOUT_SHOWDOWN
                    self.wait_timeout(effect, timeout);

                    Ok(())
                }

                // Other Holdem Stages
                _ => Ok(()),
            },

            // Other events
            _ => Ok(()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup_players() -> BTreeMap<u64, Player> {
        let mut player_map = BTreeMap::new();
        player_map.insert(1, Player::new_with_defaults(1, 0, 0, PlayerStatus::Leave));
        player_map.insert(2, Player::new_with_defaults(2, 0, 0, PlayerStatus::Out));
        player_map.insert(
            3,
            Player::new_with_defaults(3, 100, 0, PlayerStatus::Acting),
        );
        player_map
    }

    #[test]
    fn test_remove_leave_and_out_players() {
        let mut holdem = Holdem {
            player_map: setup_players(),
            ..Default::default()
        };
        let mut effect = Effect::default();
        let removed = holdem.kick_players(&mut effect);

        assert_eq!(removed.len(), 2);
        assert!(removed
            .iter()
            .any(|p| p.id == 1 && p.status == PlayerStatus::Leave));
        assert!(removed
            .iter()
            .any(|p| p.id == 2 && p.status == PlayerStatus::Out));
        assert_eq!(holdem.player_map.len(), 1);
        assert!(holdem.player_map.contains_key(&3));
    }
}
