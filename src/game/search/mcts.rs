// src/game/search/mcts.rs

use crate::game::search::{SearchConfig, Searcher};
use shakmaty::{Chess, Move, Position, Outcome, KnownOutcome};
use std::collections::HashMap;
use rand::seq::SliceRandom;

const UCT_EXPLORATION_CONSTANT: f64 = 1.41421356237; // sqrt(2)

#[derive(Clone, Debug)]
struct Node {
    visits: u32,
    wins: f64,
    parent: Option<usize>,
    children: HashMap<Move, usize>,
}

impl Node {
    fn new(parent: Option<usize>) -> Self {
        Self {
            visits: 0,
            wins: 0.0,
            parent,
            children: HashMap::new(),
        }
    }
}

pub struct MctsSearcher {
    tree: Vec<Node>,
}

impl MctsSearcher {
    pub fn new() -> Self {
        Self {
            tree: vec![Node::new(None)], // Start with a root node
        }
    }

    fn select(&self, node_index: usize, pos: &Chess) -> (usize, Chess) {
        let mut current_node_index = node_index;
        let mut current_pos = pos.clone();

        loop {
            let node = &self.tree[current_node_index];
            if node.children.is_empty() || current_pos.is_game_over() {
                return (current_node_index, current_pos);
            }

            let best_child = node.children.iter().max_by(|(_, a_idx), (_, b_idx)| {
                self.uct_value(current_node_index, **a_idx)
                    .partial_cmp(&self.uct_value(current_node_index, **b_idx))
                    .unwrap()
            });

            if let Some((best_move, best_child_index)) = best_child {
                current_pos.play_unchecked(*best_move);
                current_node_index = *best_child_index;
            } else {
                return (current_node_index, current_pos);
            }
        }
    }

    fn expand(&mut self, node_index: usize, pos: &Chess) {
        if pos.is_game_over() {
            return;
        }

        let legal_moves = pos.legal_moves();
        for m in legal_moves {
            let new_node = Node::new(Some(node_index));
            let new_node_index = self.tree.len();
            self.tree.push(new_node);
            self.tree[node_index].children.insert(m, new_node_index);
        }
    }

    fn simulate(&self, pos: &Chess) -> f64 {
        let mut sim_pos = pos.clone();
        let original_turn = sim_pos.turn();

        while !sim_pos.is_game_over() {
            let moves = sim_pos.legal_moves();

            let capture_moves: Vec<Move> = moves.iter().filter(|m| m.is_capture()).cloned().collect();

            let move_to_play = if !capture_moves.is_empty() {
                *capture_moves.choose(&mut rand::thread_rng()).unwrap()
            } else {
                *moves.choose(&mut rand::thread_rng()).unwrap()
            };

            sim_pos.play_unchecked(move_to_play);
        }

        match sim_pos.outcome() {
            Outcome::Known(KnownOutcome::Decisive { winner, .. }) => {
                if winner == original_turn { 1.0 } else { 0.0 }
            }
            Outcome::Known(KnownOutcome::Draw) => 0.5,
            _ => 0.5,
        }
    }

    fn backpropagate(&mut self, start_node_index: usize, mut result: f64) {
        let mut current_index = Some(start_node_index);
        while let Some(index) = current_index {
            self.tree[index].visits += 1;
            self.tree[index].wins += result;
            result = 1.0 - result; // Alternate result for the parent
            current_index = self.tree[index].parent;
        }
    }

    fn uct_value(&self, parent_index: usize, child_index: usize) -> f64 {
        let parent_node = &self.tree[parent_index];
        let child_node = &self.tree[child_index];

        if child_node.visits == 0 {
            return f64::INFINITY;
        }

        let exploitation = child_node.wins / child_node.visits as f64;
        let exploration = UCT_EXPLORATION_CONSTANT * ((parent_node.visits as f64).ln() / child_node.visits as f64).sqrt();

        exploitation + exploration
    }

    fn best_move(&self) -> Option<Move> {
        let root = &self.tree[0];
        root.children
            .iter()
            .max_by_key(|(_, &child_index)| self.tree[child_index].visits)
            .map(|(m, _)| *m)
    }

    fn build_move_tree_recursive(&self, node_index: usize, pos: &Chess, san: String) -> MoveTreeNode {
        let node = &self.tree[node_index];
        let mut children = Vec::new();

        for (m, &child_index) in &node.children {
            let mut new_pos = pos.clone();
            new_pos.play_unchecked(*m);
            let child_san = shakmaty::san::SanPlus::from_move(pos.clone(), *m).to_string();
            let child_node = self.build_move_tree_recursive(child_index, &new_pos, child_san);
            children.push(child_node);
        }

        MoveTreeNode {
            move_san: san,
            score: (node.wins / node.visits.max(1) as f64 * 100.0) as i32,
            children,
        }
    }
}

use super::MoveTreeNode;
use crate::app::Worker;
use std::sync::{Arc, Mutex};

impl Searcher for MctsSearcher {
    fn search(
        &mut self,
        pos: &Chess,
        _depth: u8,
        config: &SearchConfig,
        _workers: Option<Arc<Mutex<Vec<Worker>>>>,
    ) -> (Option<Move>, i32, Option<MoveTreeNode>) {
        let root_index = 0;

        for _i in 0..config.mcts_simulations {
            // 1. Selection
            let (leaf_index, leaf_pos) = self.select(root_index, pos);

            // 2. Expansion
            self.expand(leaf_index, &leaf_pos);

            // 3. Simulation
            let result = self.simulate(&leaf_pos);

            // 4. Backpropagation
            self.backpropagate(leaf_index, result);
        }

        (
            self.best_move(),
            0,
            Some(self.build_move_tree_recursive(root_index, pos, "root".to_string())),
        )
    }
}
