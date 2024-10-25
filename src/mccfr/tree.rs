use super::data::Data;
use super::info::Info;
use crate::mccfr::edge::Edge;
use crate::mccfr::node::Node;
use petgraph::graph::DiGraph;
use petgraph::graph::EdgeIndex;
use petgraph::graph::NodeIndex;
use std::fmt::Formatter;

/// Represents the game tree structure used in Monte Carlo Counterfactual Regret Minimization (MCCFR).
///
/// The `Tree` struct contains two main components:
/// 1. A directed graph (`DiGraph`) representing the game tree, where nodes are game states and edges are actions.
/// 2. A mapping from `Bucket`s to `Info`sets, which groups similar game states together.
pub struct Tree(DiGraph<Data, Edge>);

impl Tree {
    pub fn at(&self, index: NodeIndex) -> Node {
        Node::from((index, &self.0))
    }
    pub fn empty() -> Self {
        Self(DiGraph::with_capacity(0, 0))
    }
    pub fn graph(&self) -> &DiGraph<Data, Edge> {
        &self.0
    }
    pub fn insert(&mut self, spot: Data) -> NodeIndex {
        self.0.add_node(spot)
    }
    pub fn extend(&mut self, tail: NodeIndex, from: Edge, head: NodeIndex) -> EdgeIndex {
        self.0.add_edge(head, tail, from)
    }
    pub fn draw(&self, f: &mut Formatter, index: NodeIndex, prefix: &str) -> std::fmt::Result {
        if index == NodeIndex::new(0) {
            writeln!(f, "{}", self.0[index].bucket())?;
        }
        let mut children = self
            .0
            .neighbors_directed(index, petgraph::Outgoing)
            .collect::<Vec<_>>();
        children.sort();
        for (i, child) in children.iter().enumerate() {
            let last = i == children.len() - 1;
            let head = &self.0[*child].bucket();
            let edge = self
                .0
                .edge_weight(self.0.find_edge(index, *child).unwrap())
                .unwrap();
            let stem = if last { "└" } else { "├" };
            let gaps = if last { "    " } else { "│   " };
            writeln!(f, "{}{}── {} ->   {}", prefix, stem, edge, head)?;
            self.draw(f, *child, &format!("{}{}", prefix, gaps))?;
        }
        Ok(())
    }
}

impl Iterator for Tree {
    type Item = Info;
    fn next(&mut self) -> Option<Self::Item> {
        todo!()
    }
}

impl std::fmt::Display for Tree {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.draw(f, NodeIndex::new(0), "")
    }
}
