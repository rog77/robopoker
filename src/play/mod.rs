pub mod action;
pub mod game;
pub mod payout;
pub mod ply;
pub mod seat;
pub mod showdown;

pub type Chips = u16;
pub const N: usize = 2;
pub const STACK: Chips = 1_000;
