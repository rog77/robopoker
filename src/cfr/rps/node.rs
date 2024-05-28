use crate::cfr::rps::action::{Move, RpsEdge};
use crate::cfr::rps::player::RpsPlayer;
use crate::cfr::training::tree::node::Node;
use crate::cfr::training::Utility;
use std::hash::{Hash, Hasher};

use super::signal::RpsSignal;

/// Shared-lifetime game tree nodes
#[derive(PartialEq, Eq)]
pub(crate) struct RpsNode<'tree> {
    player: &'tree RpsPlayer,
    parent: Option<&'tree RpsNode<'tree>>,
    precedent: Option<&'tree RpsEdge>,
    children: Vec<&'tree RpsNode<'tree>>,
    available: Vec<&'tree RpsEdge>,
}

impl Hash for RpsNode<'_> {
    /// lucky for us, every single node in Rps has the same abstraction lookup hash, which is to say there is no information to inform your decision.
    fn hash<H: Hasher>(&self, state: &mut H) {
        0.hash(state)
    }
}

impl Node for RpsNode<'_> {
    type NPlayer = RpsPlayer;
    type NAction = RpsEdge;
    type NSignal = RpsSignal;

    fn signal(&self) -> &Self::NSignal {
        todo!("signal")
    }
    fn player(&self) -> &Self::NPlayer {
        self.player
    }
    fn available(&self) -> &Vec<&Self::NAction> {
        &self.available
    }
    fn children(&self) -> &Vec<&Self> {
        &self.children
    }
    fn parent(&self) -> &Option<&Self> {
        &self.parent
    }
    fn precedent(&self) -> &Option<&Self::NAction> {
        &self.precedent
    }
    fn utility(&self, player: &Self::NPlayer) -> Utility {
        const R_WIN: Utility = 1.0;
        const P_WIN: Utility = 1.0;
        const S_WIN: Utility = 1.0; // we can modify payoffs to verify convergence
        let a1 = self.precedent.expect("terminal node, depth = 2").turn();
        let a2 = self
            .parent
            .expect("terminal node, depth = 2")
            .precedent
            .expect("terminal node, depth = 2")
            .turn();
        let payoff = match (a1, a2) {
            (Move::R, Move::S) => R_WIN,
            (Move::R, Move::P) => -P_WIN,
            (Move::R, _) => 0.0,
            (Move::P, Move::R) => P_WIN,
            (Move::P, Move::S) => -S_WIN,
            (Move::P, _) => 0.0,
            (Move::S, Move::P) => S_WIN,
            (Move::S, Move::R) => -R_WIN,
            (Move::S, _) => 0.0,
        };
        let direction = match player {
            RpsPlayer::P1 => 0.0 + 1.0,
            RpsPlayer::P2 => 0.0 - 1.0,
        };
        direction * payoff
    }
}
