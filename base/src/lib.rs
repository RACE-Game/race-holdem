//! Holdem lib consists of the following modules:
//! - Essentials: such as Bet, Pot, Player, Street and so on
//! - Evaluator: functions and structs used in settle for comparing players' hands
//! - Game: the core logic of handling various events in the game
//!

pub mod evaluator;
pub mod account;
pub mod essential;
pub mod game;
pub mod hand_history;
pub mod errors;
