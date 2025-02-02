use super::data::Data;
use super::player::Player;
use crate::mccfr::edge::Edge;
use crate::mccfr::node::Node;
use petgraph::graph::DiGraph;
use petgraph::graph::NodeIndex;
use std::fmt::Formatter;
use std::fmt::Result;

pub struct Branch(pub Data, pub Edge, pub NodeIndex);
impl Branch {
    pub fn edge(&self) -> &Edge {
        &self.1
    }
}

/// Represents the game tree structure used in Monte Carlo Counterfactual Regret Minimization (MCCFR).
///
/// The `Tree` struct contains two main components:
/// 1. A directed graph (`DiGraph`) representing the game tree, where nodes are game states and edges are actions.
/// 2. A mapping from `Bucket`s to `Info`sets, which groups similar game states together.
#[derive(Debug, Default)]
pub struct Tree(DiGraph<Data, Edge>, Player);

impl Tree {
    pub fn all(&self) -> Vec<Node> {
        self.0.node_indices().map(|n| self.at(n)).collect()
    }
    pub fn at(&self, index: NodeIndex) -> Node {
        Node::from((index, &self.0))
    }
    pub fn empty(player: Player) -> Self {
        Self(DiGraph::with_capacity(0, 0), player)
    }
    pub fn walker(&self) -> Player {
        self.1
    }
    pub fn graph(&self) -> &DiGraph<Data, Edge> {
        &self.0
    }
    pub fn insert(&mut self, data: Data) -> Node {
        let index = self.0.add_node(data);
        let bucket = self.at(index).localization();
        self.0
            .node_weight_mut(index)
            .map(|data| data.assign(bucket))
            .expect("node index in tree");
        self.at(index)
    }
    pub fn attach(&mut self, branch: Branch) -> Node {
        let leaf = self.insert(branch.0).index();
        let edge = branch.1;
        let root = branch.2;
        self.0.add_edge(root, leaf, edge);
        self.at(leaf)
    }
    pub fn draw(&self, f: &mut Formatter, index: NodeIndex, prefix: &str) -> Result {
        if index == NodeIndex::new(0) {
            writeln!(f, "\nROOT   {}", self.at(index).bucket())?;
        }
        let mut children = self
            .0
            .neighbors_directed(index, petgraph::Outgoing)
            .collect::<Vec<_>>();
        let n = children.len();
        children.sort();
        for (i, child) in children.into_iter().enumerate() {
            let last = i == n - 1;
            let stem = if last { "└" } else { "├" };
            let gaps = if last { "    " } else { "│   " };
            let node = self.at(child);
            let head = node.bucket();
            let edge = self
                .0
                .edge_weight(self.0.find_edge(index, child).unwrap())
                .unwrap();
            writeln!(f, "{}{}──{} → {}", prefix, stem, edge, head)?;
            self.draw(f, child, &format!("{}{}", prefix, gaps))?;
        }
        Ok(())
    }
}

impl std::fmt::Display for Tree {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.draw(f, NodeIndex::new(0), "")
    }
}
