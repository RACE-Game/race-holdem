//! Game state machine (or handler) of Holdem: the core of this lib.
use race_api::prelude::*;
use std::collections::BTreeMap;

use crate::errors;
use crate::essential::{
    ActingPlayer, AwardPot, Display, GameEvent, GameMode, HoldemAccount, HoldemStage,
    InternalPlayerJoin, Player, PlayerResult, PlayerStatus, Pot, Street, ACTION_TIMEOUT_POSTFLOP,
    ACTION_TIMEOUT_PREFLOP, ACTION_TIMEOUT_RIVER, ACTION_TIMEOUT_TURN, MAX_ACTION_TIMEOUT_COUNT,
    WAIT_TIMEOUT_DEFAULT, WAIT_TIMEOUT_LAST_PLAYER, WAIT_TIMEOUT_RUNNER, WAIT_TIMEOUT_SHOWDOWN,
};
use crate::evaluator::{compare_hands, create_cards, evaluate_cards, PlayerHand};
use crate::hand_history::{BlindInfo, BlindType, HandHistory, PlayerAction, Showdown};

// Holdem: the game state
#[derive(BorshSerialize, BorshDeserialize, Default, Debug, PartialEq)]
pub struct Holdem {
    pub deck_random_id: RandomId,
    pub sb: u64,
    pub bb: u64,
    pub min_raise: u64,
    pub btn: usize,
    pub rake: u16,
    pub stage: HoldemStage,
    pub street: Street,
    pub street_bet: u64,
    pub board: Vec<String>,
    pub hand_index_map: BTreeMap<String, Vec<usize>>,
    pub bet_map: BTreeMap<String, u64>,
    pub total_bet_map: BTreeMap<String, u64>,
    pub prize_map: BTreeMap<String, u64>,
    pub player_map: BTreeMap<String, Player>,
    pub player_order: Vec<String>,
    pub pots: Vec<Pot>,
    pub acting_player: Option<ActingPlayer>,
    pub winners: Vec<String>,
    pub display: Vec<Display>,
    pub mode: GameMode,
    pub next_game_start: u64, // A timestamp indicates when the next game will start
    pub table_size: u8,       // The size of table
    pub hand_history: HandHistory,
}

#[derive(BorshSerialize, BorshDeserialize)]
pub struct HoldemCheckpoint {
    pub btn: usize,
    pub player_timeouts: BTreeMap<String, u8>,
}

impl From<Holdem> for HoldemCheckpoint {
    fn from(value: Holdem) -> Self {
        let Holdem {
            player_map, btn, ..
        } = value;

        let player_timeouts = player_map
            .into_iter()
            .map(|p| (p.0, p.1.timeout))
            .collect::<BTreeMap<String, u8>>();

        Self {
            btn,
            player_timeouts,
        }
    }
}

// Methods that mutate or query the game state
impl Holdem {
    // Mark out players.
    // An out player is one with zero chips.
    fn mark_out_players(&mut self) {
        for (_, v) in self.player_map.iter_mut() {
            if v.status != PlayerStatus::Leave && v.chips == 0 {
                v.status = PlayerStatus::Out;
                // Here we use timeout for rebuy timeout.
                v.timeout = 0;
            };
        }
    }

    // Remove players with `Leave` or `Out` status.
    fn remove_leave_and_out_players(&mut self) -> Vec<String> {
        let mut removed = Vec::with_capacity(self.player_map.len());
        self.player_map.retain(|_, p| {
            if p.status == PlayerStatus::Leave || p.status == PlayerStatus::Out {
                removed.push(p.addr.clone());
                false
            } else {
                true
            }
        });
        println!("Remove these players: {:?}", removed);
        removed
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
    fn signal_game_end(&mut self) -> Result<(), HandleError> {
        self.street_bet = 0;
        self.min_raise = 0;
        self.acting_player = None;

        Ok(())
    }

    pub fn reset_holdem_state(&mut self) -> Result<(), HandleError> {
        self.winners.clear();
        self.street = Street::Init;
        self.stage = HoldemStage::Init;
        self.pots.clear();
        self.board.clear();
        self.player_order.clear();
        self.hand_index_map.clear();
        self.bet_map.clear();
        self.total_bet_map.clear();
        self.prize_map.clear();
        self.next_game_start = 0;

        Ok(())
    }

    /// Return the next acting player
    fn next_action_player(&mut self, next_players: Vec<&String>) -> Option<String> {
        for addr in next_players {
            if let Some(player) = self.player_map.get(addr) {
                let curr_bet: u64 = self.bet_map.get(addr).map(|b| *b).unwrap_or(0);
                if curr_bet < self.street_bet || player.status == PlayerStatus::Wait {
                    return Some(addr.clone());
                }
            }
        }
        None
    }

    pub fn is_acting_player(&self, player_addr: &str) -> bool {
        match &self.acting_player {
            Some(ActingPlayer { addr, .. }) => addr == player_addr,
            None => false,
        }
    }

    fn get_remainder_player(&mut self) -> Option<String> {
        let eligible_candidates = {
            let mut players = self
                .player_map
                .values()
                .filter(|p| {
                    self.prize_map.contains_key(&p.addr)
                        && matches!(
                            p.status,
                            PlayerStatus::Acted | PlayerStatus::Allin | PlayerStatus::Wait
                        )
                })
                .map(|p| (p.addr(), p.position))
                .collect::<Vec<(String, usize)>>();
            players.sort_by(|(_, pos1), (_, pos2)| pos1.cmp(pos2));
            players
                .into_iter()
                .map(|(addr, _)| addr)
                .collect::<Vec<String>>()
        };

        let remainder_player = if eligible_candidates.is_empty() {
            // When no remainder player, use the the first in player map
            self.player_map
                .first_key_value()
                .and_then(|(addr, _)| Some(addr.clone()))
        } else {
            eligible_candidates
                .first()
                .and_then(|addr| Some(addr.clone()))
        };

        remainder_player
    }

    /// Return either acting player position or btn for reference
    fn get_ref_position(&self) -> usize {
        if let Some(ActingPlayer {
            addr: _,
            position,
            clock: _,
        }) = self.acting_player
        {
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
                return Err(HandleError::Custom(
                    "Failed to find a player for the next button".to_string(),
                ));
            };
            Ok(*next_btn)
        } else {
            if let Some(next_btn) = next_positions.first() {
                Ok(*next_btn)
            } else {
                return Err(HandleError::Custom(
                    "Failed to find a proper position for the next button".to_string(),
                ));
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

    pub fn ask_for_action(
        &mut self,
        player_addr: String,
        effect: &mut Effect,
    ) -> Result<(), HandleError> {
        let timeout = self.get_action_time();
        if let Some(player) = self.player_map.get_mut(&player_addr) {
            println!("Asking {} to act", player.addr);
            player.status = PlayerStatus::Acting;
            self.acting_player = Some(ActingPlayer {
                addr: player.addr(),
                position: player.position,
                clock: effect.timestamp() + timeout,
            });
            if self.mode != GameMode::Mtt {
                effect.action_timeout(player_addr, timeout); // in msecs
            }
            Ok(())
        } else {
            return Err(HandleError::Custom(
                "Next player not found in game".to_string(),
            ));
        }
    }

    /// According to players position, place them in the following order:
    /// SB, BB, UTG (1st-to-act), MID (2nd-to-act), ..., BTN (last-to-act).
    pub fn arrange_players(&mut self, last_pos: usize) -> Result<(), HandleError> {
        let mut player_pos: Vec<(String, usize)> = self
            .player_map
            .values()
            .filter(|p| p.status != PlayerStatus::Init)
            .map(|p| {
                if p.position > last_pos {
                    (p.addr(), p.position - last_pos)
                } else {
                    (p.addr(), p.position + 100)
                }
            })
            .collect();
        player_pos.sort_by(|(_, pos1), (_, pos2)| pos1.cmp(pos2));
        let player_order: Vec<String> = player_pos.into_iter().map(|(addr, _)| addr).collect();
        println!("Player order {:?}", player_order);
        self.player_order = player_order;
        Ok(())
    }

    pub fn blind_bets(&mut self, effect: &mut Effect) -> Result<(), HandleError> {
        let (sb_addr, bb_addr) = if self.player_order.len() == 2 {
            let bb_addr = self
                .player_order
                .first()
                .cloned()
                .ok_or(errors::heads_up_missing_sb())?;
            let sb_addr = self
                .player_order
                .last()
                .cloned()
                .ok_or(errors::heads_up_missing_bb())?;
            (sb_addr, bb_addr)
        } else {
            let sb_addr = self
                .player_order
                .get(0)
                .cloned()
                .ok_or(errors::mplayers_missing_sb())?;
            let bb_addr = self
                .player_order
                .get(1)
                .cloned()
                .ok_or(errors::mplayers_missing_bb())?;
            (sb_addr, bb_addr)
        };

        let (allin, real_sb) = self.take_bet(sb_addr.to_owned(), self.sb)?;
        if allin {
            self.set_player_status(&sb_addr, PlayerStatus::Allin)?;
        }
        let (allin, real_bb) = self.take_bet(bb_addr.to_owned(), self.bb)?;
        if allin {
            self.set_player_status(&bb_addr, PlayerStatus::Allin)?;
        }

        let hh = &mut self.hand_history;
        hh.set_blinds_infos(vec![
            BlindInfo::new(&sb_addr, BlindType::Sb, real_sb),
            BlindInfo::new(&bb_addr, BlindType::Bb, real_bb),
        ]);
        hh.set_pot(Street::Preflop, real_sb + real_bb);

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
            None => return Err(errors::internal_cannot_find_action_player()),
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
        // Remove any folded players from owners of a pot
        let unfolded_player_addrs: Vec<String> = self
            .player_map
            .values()
            .filter(|p| {
                matches!(
                    p.status,
                    PlayerStatus::Wait | PlayerStatus::Allin | PlayerStatus::Acted
                )
            })
            .map(|p| p.addr.clone())
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
            let mut owners: Vec<String> = self
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

                new_pots.push(Pot {
                    owners,
                    winners: Vec::<String>::new(),
                    amount,
                });
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
        self.display.push(Display::CollectBets {
            bet_map: self.bet_map.clone(),
        });
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
        println!("Street changes to {:?}", self.street);
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
                println!("Board is {:?}", self.board);
            }

            Street::Turn => {
                effect.reveal(self.deck_random_id, vec![players_cnt + 3]);
                self.stage = HoldemStage::ShareKey;
                println!("Board is {:?}", self.board);
            }

            Street::River => {
                effect.reveal(self.deck_random_id, vec![players_cnt + 4]);
                self.stage = HoldemStage::ShareKey;
                println!("Board is {:?}", self.board);
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
                println!("Board is {:?}", self.board);
            }
            _ => {}
        }
        Ok(())
    }

    /// Take the rake from winners' pot and update prize map.
    pub fn take_rake_from_prize(&mut self) -> Result<u64, HandleError> {
        // Only take rakes in Cash game
        if self.mode != GameMode::Cash {
            return Ok(0);
        }

        // No rake for preflop
        if self.street == Street::Preflop {
            return Ok(0);
        }
        let mut total_rake = 0;

        // For now, we use 5BB as rake cap
        let rake_cap = self.bb * 5;

        for (_, prize) in self.prize_map.iter_mut() {
            if *prize > 0 {
                let r = u64::min(self.rake as u64 * *prize / 1000u64, rake_cap);
                total_rake += r;
                *prize = prize
                    .checked_sub(r)
                    .ok_or(errors::internal_amount_overflow())?;
            }
        }

        return Ok(total_rake);
    }

    /// Build the prize map for awarding chips
    pub fn calc_prize(&mut self) -> Result<(), HandleError> {
        let pots = &mut self.pots;
        let mut prize_map = BTreeMap::<String, u64>::new();
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
                    .entry(winner.clone())
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
            match self.prize_map.get(&player.addr) {
                Some(prize) => {
                    player.chips += *prize;
                    println!("Player {} won {} chips", player.addr, *prize);
                }
                None => {
                    println!("Player {} lost the bet", player.addr);
                }
            }
        }
        Ok(())
    }

    /// winner_sets:
    /// examples: [[alice, bob], [charlie, dave]] can be used to represent Royal flush: alice, bob > Flush: charlie, dave
    pub fn assign_winners(&mut self, winner_sets: Vec<Vec<String>>) -> Result<(), HandleError> {
        for pot in self.pots.iter_mut() {
            for winner_set in winner_sets.iter() {
                let real_winners: Vec<String> = winner_set
                    .iter()
                    .filter(|&w| pot.owners.contains(w))
                    .map(|w| w.clone())
                    .collect();
                // A pot should have at least one winner
                if real_winners.len() >= 1 {
                    for w in real_winners.iter() {
                        let Some(_player) = self.player_map.get_mut(w) else {
                            return Err(HandleError::Custom(
                                "Winner not found in player map".to_string()
                            ));
                        };
                    }
                    pot.winners = real_winners;
                    break;
                } else {
                    continue;
                }
            }

            if pot.winners.is_empty() {
                return Err(HandleError::Custom("Winner not found".to_string()));
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
        self.display.push(Display::AwardPots { pots: award_pots });

        Ok(())
    }

    /// Update the map that records players chips change (increased or decreased)
    /// Used for settlement
    pub fn update_chips_map(&mut self) -> Result<BTreeMap<String, i64>, HandleError> {
        // The i64 change for each player.  The amount = total pots
        // earned - total bet.  This map will be returned for furture
        // calculation.
        let mut chips_change_map: BTreeMap<String, i64> = self
            .player_map
            .keys()
            .map(|addr| (addr.clone(), 0))
            .collect();

        // The players for game result information.  The `chips` is
        // the amount before the settlement, the `prize` is the sum of
        // pots earned during the settlement.  This map will be added
        // to display.
        let mut result_player_map = BTreeMap::<String, PlayerResult>::new();

        self.winners = Vec::<String>::with_capacity(self.player_map.len());

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

        for (addr, player) in self.player_map.iter() {
            let prize = if let Some(p) = self.prize_map.get(addr).copied() {
                if p == 0 {
                    None
                } else {
                    Some(p)
                }
            } else {
                None
            };

            let result = PlayerResult {
                addr: addr.clone(),
                position: player.position,
                status: player.status,
                chips: player.chips,
                prize,
            };

            result_player_map.insert(addr.clone(), result);
        }

        self.display.push(Display::GameResult {
            player_map: result_player_map,
        });

        self.hand_history.set_chips_change(&chips_change_map);
        Ok(chips_change_map)
    }

    pub fn single_player_win(
        &mut self,
        effect: &mut Effect,
        winner: String,
    ) -> Result<(), HandleError> {
        self.collect_bets()?;
        self.assign_winners(vec![vec![winner]])?;
        self.calc_prize()?;
        let rake = self.take_rake_from_prize()?;
        let chips_change_map = self.update_chips_map()?;
        self.apply_prize()?;

        // Add or reduce players chips according to chips change map
        for (player, chips_change) in chips_change_map.iter() {
            if *chips_change > 0 {
                effect.settle(Settle::add(player, *chips_change as u64));
            } else if *chips_change < 0 {
                effect.settle(Settle::sub(player, -*chips_change as u64));
            }
        }

        self.mark_out_players();

        let removed_addrs = self.remove_leave_and_out_players();
        for addr in removed_addrs {
            effect.settle(Settle::eject(addr));
        }

        if rake > 0 {
            effect.transfer(0, rake);
        }

        self.wait_timeout(effect, WAIT_TIMEOUT_LAST_PLAYER);
        effect.checkpoint();
        Ok(())
    }

    pub fn wait_timeout(&mut self, effect: &mut Effect, timeout: u64) {
        self.next_game_start = effect.timestamp() + timeout;

        if self.mode != GameMode::Mtt {
            effect.wait_timeout(timeout);
        }
    }

    pub fn settle(&mut self, effect: &mut Effect) -> Result<(), HandleError> {
        let decryption = effect.get_revealed(self.deck_random_id)?;
        // Board
        let board: Vec<&str> = self.board.iter().map(|c| c.as_str()).collect();
        // Player hands
        let mut player_hands: Vec<(String, PlayerHand)> =
            Vec::with_capacity(self.player_order.len());

        let mut showdowns = Vec::<(String, Showdown)>::new();

        for (addr, idxs) in self.hand_index_map.iter() {
            if idxs.len() != 2 {
                return Err(HandleError::Custom(
                    "Invalid hole-card idx vec: length not equal to 2".to_string(),
                ));
            }

            let Some(player) = self.player_map.get(addr) else {
                return Err(HandleError::Custom(
                    "Player not found in game [settle]".to_string()
                ));
            };

            if player.status != PlayerStatus::Fold
                && player.status != PlayerStatus::Init
                && player.status != PlayerStatus::Leave
            {
                let Some(first_card_idx) = idxs.first() else {
                    return Err(HandleError::Custom(
                        "Failed to extract index for 1st hole card".to_string()
                    ));
                };
                let Some(first_card) = decryption.get(first_card_idx) else {
                    return Err(HandleError::Custom(
                        format!("Failed to get revealed info, index: {}, avaliable indexes: {:?}",
                        first_card_idx, decryption.keys().collect::<Vec<&usize>>())
                    ));
                };
                let Some(second_card_idx) = idxs.last() else {
                    return Err(HandleError::Custom(
                        "Failed to extract index for 2nd hole card".to_string()
                    ));
                };
                let Some(second_card) = decryption.get(second_card_idx) else {
                    return Err(HandleError::Custom(
                        "Failed to get 2nd hole card from the revealed info".to_string()
                    ));
                };
                let hole_cards = [first_card.as_str(), second_card.as_str()];
                let cards = create_cards(board.as_slice(), &hole_cards);
                let hand = evaluate_cards(cards);
                let hole_cards = hole_cards.iter().map(|c| c.to_string()).collect();
                let category = hand.category.clone();
                let picks = hand.picks.iter().map(|c| c.to_string()).collect();
                player_hands.push((player.addr(), hand));
                showdowns.push((
                    player.addr(),
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
        let mut winners: Vec<Vec<String>> = Vec::new();
        let mut weaker: Vec<Vec<String>> = Vec::new();
        // Players in a draw will be in the same set
        let mut draws = Vec::<String>::new();
        // Each hand is either equal to or weaker than winner (1st)
        let Some((winner, highest_hand)) = player_hands.first() else {
            return Err(HandleError::Custom(
                "Failed to spot the strongest hand".to_string()
            ));
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

        self.assign_winners(winners)?;
        self.calc_prize()?;
        let rake = self.take_rake_from_prize()?;
        let chips_change_map = self.update_chips_map()?;
        self.apply_prize()?;

        // Add or reduce players chips according to chips change map
        for (player, chips_change) in chips_change_map.iter() {
            if *chips_change > 0 {
                effect.settle(Settle::add(player, *chips_change as u64))
            } else if *chips_change < 0 {
                effect.settle(Settle::sub(player, -*chips_change as u64))
            }
        }

        self.mark_out_players();
        let removed_addrs = self.remove_leave_and_out_players();

        if self.mode == GameMode::Cash {
            for addr in removed_addrs {
                effect.settle(Settle::eject(addr));
            }
        }

        if rake > 0 {
            effect.transfer(0, rake);
        }

        effect.checkpoint();

        // Save to hand history
        for (addr, showdown) in showdowns.into_iter() {
            self.hand_history.add_showdown(addr, showdown);
        }
        Ok(())
    }

    // De facto entry point of Holdem
    pub fn next_state(&mut self, effect: &mut Effect) -> Result<(), HandleError> {
        let last_pos = self.get_ref_position();
        self.arrange_players(last_pos)?;
        // ingame_players exclude anyone with `Init` status
        let ingame_players = self.player_order.clone();
        let mut players_to_stay = Vec::<&String>::new();
        let mut players_to_act = Vec::<&String>::new();
        let mut players_allin = Vec::<&String>::new();

        for addr in ingame_players.iter() {
            if let Some(player) = self.player_map.get(addr) {
                match player.status {
                    PlayerStatus::Acting => {
                        players_to_stay.push(addr);
                    }
                    PlayerStatus::Wait | PlayerStatus::Acted => {
                        players_to_stay.push(addr);
                        players_to_act.push(addr);
                    }
                    PlayerStatus::Allin => {
                        players_to_stay.push(addr);
                        players_allin.push(addr);
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
            self.signal_game_end()?;
            let Some(winner) = ingame_players.first() else {
                return Err(HandleError::Custom(
                    "Failed to get the only player".to_string()
                ));
            };
            println!("[Next State]: Single winner: {}", winner);
            self.single_player_win(effect, winner.clone())?;
            Ok(())
        }
        // Single players wins because others all folded
        else if players_to_stay.len() == 1 {
            self.stage = HoldemStage::Settle;
            self.signal_game_end()?;
            let Some(winner) = players_to_stay.first() else {
                return Err(HandleError::Custom(
                    "Failed to get the single winner left".to_string()
                ));
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
                return Err(HandleError::Custom(
                    "Failed to get the next-to-act player".to_string()
                ));
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
            self.signal_game_end()?;
            self.collect_bets()?;

            // Reveal all cards for eligible players: not folded and without init status
            for (addr, idxs) in self.hand_index_map.iter() {
                let Some(player) = self.player_map.get(addr) else {
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
            self.signal_game_end()?;
            self.collect_bets()?;

            // Reveal players' hole cards
            for (addr, idxs) in self.hand_index_map.iter() {
                let Some(player) = self.player_map.get(addr) else {
                    return Err(HandleError::Custom(
                        "Player not found in game but assigned cards".to_string()
                    ));
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
        sender: String,
    ) -> Result<(), HandleError> {
        self.display.clear();

        let Some(player) = self.player_map.get(&sender) else {
            return Err(HandleError::InvalidPlayer)
        };

        match event {
            GameEvent::Bet(amount) => {
                if !self.is_acting_player(&sender) {
                    return Err(errors::not_the_acting_player_to_bet());
                }
                if self.bet_map.get(&sender).is_some() {
                    return Err(errors::player_already_betted());
                }
                // Freestyle betting not allowed in the preflop
                if self.street_bet != 0 {
                    return Err(errors::player_cant_bet());
                }
                if self.bb > amount {
                    return Err(errors::bet_amonut_is_too_small());
                }

                let (allin, _) = self.take_bet(sender.clone(), amount)?;
                self.set_player_acted(&sender, allin)?;
                self.min_raise = amount;
                self.street_bet = amount;
            }

            GameEvent::Call => {
                if !self.is_acting_player(&sender) {
                    return Err(errors::not_the_acting_player_to_call());
                }

                let betted = self.get_player_bet(&sender);
                let call_amount = self.street_bet - betted;
                let (allin, _) = self.take_bet(sender.clone(), call_amount)?;
                self.set_player_acted(&sender, allin)?;
            }

            GameEvent::Check => {
                if !self.is_acting_player(&sender) {
                    return Err(errors::not_the_acting_player_to_check());
                }

                // Check is only available when player's current bet equals street bet.
                let curr_bet = self.get_player_bet(&sender);
                if curr_bet != self.street_bet {
                    return Err(errors::player_cant_check());
                }
                self.set_player_status(&sender, PlayerStatus::Acted)?;
            }

            GameEvent::Fold => {
                if !self.is_acting_player(&sender) {
                    return Err(errors::not_the_acting_player_to_fold());
                }
                self.set_player_status(&sender, PlayerStatus::Fold)?;
            }

            GameEvent::Raise(amount) => {
                if !self.is_acting_player(&sender) {
                    return Err(errors::not_the_acting_player_to_raise());
                }

                if self.street_bet == 0 || self.bet_map.is_empty() {
                    return Err(errors::player_cant_raise());
                }

                let betted = self.get_player_bet(&sender);
                if amount + betted < self.street_bet + self.min_raise && amount != player.chips {
                    return Err(errors::raise_amount_is_too_small());
                }
                let (allin, real_bet) = self.take_bet(sender.clone(), amount)?;
                self.set_player_acted(&sender, allin)?;
                let new_street_bet = betted + real_bet;
                let new_min_raise = new_street_bet - self.street_bet;
                self.street_bet = new_street_bet;
                self.min_raise = new_min_raise;
            }
        }

        // Save action to hand history
        let street = self.street;
        self.hand_history
            .add_action(street, PlayerAction::new(&sender, event))?;
        self.next_state(effect)?;
        Ok(())
    }

    pub fn set_player_acted(&mut self, player_addr: &str, allin: bool) -> Result<(), HandleError> {
        self.set_player_status(
            player_addr,
            if allin {
                PlayerStatus::Allin
            } else {
                PlayerStatus::Acted
            },
        )
    }

    pub fn reset_player_timeout(&mut self, player_addr: &str) -> Result<(), HandleError> {
        let Some(player) = self.player_map.get_mut(player_addr) else {
            return Err(HandleError::InvalidPlayer);
        };
        player.timeout = 0;
        Ok(())
    }

    pub fn set_player_status(
        &mut self,
        player_addr: &str,
        status: PlayerStatus,
    ) -> Result<(), HandleError> {
        let Some(player) = self.player_map.get_mut(player_addr) else {
            return Err(HandleError::InvalidPlayer);
        };
        player.status = status;
        Ok(())
    }

    pub fn get_player_bet(&self, player_addr: &str) -> u64 {
        self.bet_map.get(player_addr).cloned().unwrap_or(0)
    }

    pub fn take_bet(
        &mut self,
        player_addr: String,
        amount: u64,
    ) -> Result<(bool, u64), HandleError> {
        let Some(player) = self.player_map.get_mut(&player_addr) else {
            return Err(HandleError::InvalidPlayer);
        };
        let (allin, real_amount) = player.take_bet(amount);
        self.bet_map
            .entry(player_addr.clone())
            .and_modify(|amt| *amt += real_amount)
            .or_insert(real_amount);
        self.total_bet_map
            .entry(player_addr)
            .and_modify(|amt| *amt += real_amount)
            .or_insert(real_amount);
        Ok((allin, real_amount))
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
                return Err(HandleError::Custom("Table is full".to_string()))
            };

            self.player_map.insert(
                p.addr.clone(),
                Player::new_with_status(p.addr, p.chips, pos, PlayerStatus::Fold),
            );
        }
        Ok(())
    }

    pub fn internal_start_game(&mut self, effect: &mut Effect) -> Result<(), HandleError> {
        // WaitingTimeout + GameStart
        self.display.clear();

        let next_btn = self.get_next_btn()?;
        println!("Next BTN: {}", next_btn);
        self.btn = next_btn;

        let player_num = self.player_map.len();
        println!("{} players in game", player_num);

        if player_num >= 2 {
            // Prepare randomness (shuffling cards)
            let rnd_spec = RandomSpec::deck_of_cards();
            self.deck_random_id = effect.init_random_state(rnd_spec);
        }

        // Init HandHistory
        self.hand_history = HandHistory::default();

        Ok(())
    }
}

impl GameHandler for Holdem {
    type Checkpoint = HoldemCheckpoint;

    fn init_state(effect: &mut Effect, init_account: InitAccount) -> Result<Self, HandleError> {
        let HoldemAccount { sb, bb, rake, .. } = init_account.data()?;
        let checkpoint: Option<HoldemCheckpoint> = init_account.checkpoint()?;
        let (player_timeouts, btn) = if let Some(checkpoint) = checkpoint {
            (checkpoint.player_timeouts, checkpoint.btn)
        } else {
            (BTreeMap::default(), 0)
        };

        let player_map: BTreeMap<String, Player> = init_account
            .players
            .iter()
            .map(|p| {
                let addr = p.addr.clone();
                let timeout = player_timeouts.get(&addr).cloned().unwrap_or_default();
                let player = Player::new(p.addr.clone(), p.balance, p.position, timeout);
                (addr, player)
            })
            .collect();

        effect.allow_exit(true);

        Ok(Self {
            deck_random_id: 0,
            sb,
            bb,
            min_raise: bb,
            btn,
            rake,
            stage: HoldemStage::Init,
            street: Street::Init,
            street_bet: 0,
            board: Vec::<String>::with_capacity(5),
            hand_index_map: BTreeMap::<String, Vec<usize>>::new(),
            bet_map: BTreeMap::<String, u64>::new(),
            total_bet_map: BTreeMap::<String, u64>::new(),
            prize_map: BTreeMap::<String, u64>::new(),
            player_map,
            player_order: Vec::<String>::new(),
            pots: Vec::<Pot>::new(),
            acting_player: None,
            winners: Vec::<String>::new(),
            display: Vec::<Display>::new(),
            mode: GameMode::Cash,
            next_game_start: 0,
            table_size: init_account.max_players as u8,
            hand_history: HandHistory::default(),
        })
    }

    fn handle_event(&mut self, effect: &mut Effect, event: Event) -> Result<(), HandleError> {

        match event {
            // Handle holdem specific (custom) events
            Event::Custom { sender, raw } => {
                self.display.clear();
                self.reset_player_timeout(&sender)?;
                let event: GameEvent = GameEvent::try_parse(&raw)?;
                println!("Player action event: {:?}, sender: {:?}", event, sender);
                self.handle_custom_event(effect, event, sender.clone())?;
                Ok(())
            }

            Event::ActionTimeout { player_addr } => {
                self.display.clear();

                if !self.is_acting_player(&player_addr) {
                    return Err(HandleError::Custom("Player is not acting".to_string()));
                }

                let Some(player) = self.player_map.get_mut(&player_addr) else {
                    return Err(HandleError::Custom("Player not found in game".to_string()));
                };

                let street = self.street;
                // In Cash game, mark those who've reached T/O for
                // MAX_ACTION_TIMEOUT_COUNT times with `Leave` status
                if self.mode == GameMode::Cash {
                    if player.timeout >= MAX_ACTION_TIMEOUT_COUNT {
                        player.status = PlayerStatus::Leave;
                        self.hand_history.add_action(
                            street,
                            PlayerAction {
                                addr: player_addr.to_owned(),
                                event: GameEvent::Fold,
                            },
                        )?;
                        self.next_state(effect)?;
                        return Ok(());
                    } else {
                        player.timeout += 1;
                    }
                }

                let street_bet = self.street_bet;
                let bet = if let Some(player_bet) = self.bet_map.get(&player_addr) {
                    *player_bet
                } else {
                    0
                };

                if bet == street_bet {
                    self.set_player_status(&player_addr, PlayerStatus::Acted)?;
                    self.hand_history.add_action(
                        street,
                        PlayerAction {
                            addr: player_addr.to_owned(),
                            event: GameEvent::Check,
                        },
                    )?;
                    self.next_state(effect)?;
                    Ok(())
                } else {
                    self.set_player_status(&player_addr, PlayerStatus::Fold)?;
                    self.hand_history.add_action(
                        street,
                        PlayerAction {
                            addr: player_addr.to_owned(),
                            event: GameEvent::Fold,
                        },
                    )?;
                    self.next_state(effect)?;
                    Ok(())
                }
            }

            Event::WaitingTimeout | Event::Ready => {
                self.display.clear();
                self.reset_holdem_state()?;
                self.reset_player_map_status()?;

                if effect.count_players() >= 2 && effect.count_servers() >= 1 {
                    self.next_game_start = 0;
                    effect.start_game();
                }
                Ok(())
            }

            Event::Sync { new_players, .. } => {
                self.display.clear();
                match self.stage {
                    HoldemStage::Init => {
                        for p in new_players.into_iter() {
                            let PlayerJoin {
                                addr,
                                position,
                                balance,
                                ..
                            } = p;
                            let player = Player::new(addr, balance, position, 0);
                            self.player_map.insert(player.addr(), player);
                        }

                        if effect.count_players() >= 2 && effect.count_servers() >= 1 {
                            self.next_game_start = 0;
                            effect.start_game();
                        }
                    }

                    _ => {
                        for p in new_players.into_iter() {
                            let PlayerJoin {
                                addr,
                                position,
                                balance,
                                ..
                            } = p;
                            let player = Player::init(addr, balance, position);
                            self.player_map.insert(player.addr(), player);
                        }
                    }
                }

                Ok(())
            }

            Event::GameStart { .. } => {
                self.next_game_start = 0;
                self.display.clear();

                let next_btn = self.get_next_btn()?;
                println!("Game starts and next BTN: {}", next_btn);
                self.btn = next_btn;

                // Prepare randomness (shuffling cards)
                let rnd_spec = RandomSpec::deck_of_cards();
                self.deck_random_id = effect.init_random_state(rnd_spec);

                // Init HandHistory
                self.hand_history = HandHistory::default();

                Ok(())
            }

            Event::Leave { player_addr } => {
                // TODO: Leaving is not allowed in SNG game
                self.display.clear();
                println!("Player {} decides to leave game", player_addr);

                let Some(leaving_player) = self.player_map.get_mut(&player_addr) else {
                    return Err(HandleError::Custom(
                        "Player not found in game [Leave]".to_string()
                    ));
                };
                leaving_player.status = PlayerStatus::Leave;

                match self.stage {
                    // If current stage is not playing, the player can
                    // leave with a settlement instantly.
                    HoldemStage::Init
                    | HoldemStage::Settle
                    | HoldemStage::Runner
                    | HoldemStage::Showdown => {
                        self.player_map.remove_entry(&player_addr);
                        effect.settle(Settle::eject(&player_addr));
                        effect.checkpoint();
                        self.wait_timeout(effect, WAIT_TIMEOUT_DEFAULT);
                        self.signal_game_end()?;
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
                        if self.stage == HoldemStage::Play
                            && !self.is_acting_player(&player_addr)
                            && self.count_unfolded_players() > 1
                        {
                            println!("Game continue as the folded player is not the acting player");
                        } else {
                            self.next_state(effect)?;
                        }
                    }
                }

                Ok(())
            }

            Event::RandomnessReady { .. } => {
                self.display.clear();
                // Cards are dealt to players but remain invisible to them
                for (idx, (addr, player)) in self.player_map.iter().enumerate() {
                    if player.status != PlayerStatus::Init {
                        effect.assign(self.deck_random_id, addr, vec![idx * 2, idx * 2 + 1]);
                        self.hand_index_map
                            .insert(addr.clone(), vec![idx * 2, idx * 2 + 1]);
                    }
                }

                Ok(())
            }

            Event::SecretsReady { .. } => match self.stage {
                HoldemStage::ShareKey => {
                    self.display.clear();
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
                                    return Err(HandleError::Custom(
                                        "Failed to reveal the 3 flop cards".to_string(),
                                    ));
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
                                return Err(HandleError::Custom(
                                    "Failed to reveal the turn card".to_string(),
                                ));
                            }

                            self.hand_history.set_board(self.board.clone());
                            self.next_state(effect)?;
                        }

                        Street::River => {
                            let decryption = effect.get_revealed(self.deck_random_id)?;
                            let card_index = players_cnt + 4;
                            if let Some(card) = decryption.get(&card_index) {
                                self.board.push(card.clone());
                                self.display.push(Display::DealBoard {
                                    prev: board_prev_cnt,
                                    board: self.board.clone(),
                                });
                            } else {
                                return Err(HandleError::Custom(
                                    "Failed to reveal the river card".to_string(),
                                ));
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
                    self.display.clear();
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
                    self.display.clear();
                    let prev_board_cnt = self.board.len();
                    self.update_board(effect)?;
                    self.display.push(Display::DealBoard {
                        prev: prev_board_cnt,
                        board: self.board.clone(),
                    });
                    self.settle(effect)?;

                    self.wait_timeout(effect, WAIT_TIMEOUT_RUNNER);
                    Ok(())
                }

                // Ending, comparing cards
                HoldemStage::Showdown => {
                    self.display.clear();
                    self.settle(effect)?;
                    self.wait_timeout(effect, WAIT_TIMEOUT_SHOWDOWN);
                    Ok(())
                }

                // Other Holdem Stages
                _ => Ok(()),
            },

            // Other events
            _ => Ok(()),
        }
    }

    /// The implementation depends on the game type
    fn into_checkpoint(self) -> HandleResult<HoldemCheckpoint> {
        Ok(self.into())
    }
}
