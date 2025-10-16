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
            } else if let Some(m) = moves.choose(&mut rand::thread_rng()){
                *m
            } else {
                break;
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
            let original_pos = pos.clone();
            if let Ok(new_pos) = pos.clone().play(*m) {
                let child_san = shakmaty::san::SanPlus::from_move(original_pos, *m).to_string();
                let child_node = self.build_move_tree_recursive(child_index, &new_pos, child_san);
                children.push(child_node);
            }
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
use crate::ga::EvolutionUpdate;
use crossbeam_channel::Sender;
use std::sync::{Arc, Mutex};
use std::time::Instant;
use crossbeam_utils::thread;
use num_cpus;

impl Searcher for MctsSearcher {
    fn search(
        &mut self,
        pos: &Chess,
        _depth: u8,
        config: &SearchConfig,
        workers: Option<Arc<Mutex<Vec<Worker>>>>,
        update_sender: Option<Sender<EvolutionUpdate>>,
    ) -> (Option<Move>, i32, Option<MoveTreeNode>) {
        let num_threads = num_cpus::get();
        let simulations_per_thread = (config.mcts_simulations as usize / num_threads).max(1);
        let (tx, rx) = std::sync::mpsc::channel();

        thread::scope(|s| {
            for i in 0..num_threads {
                let pos = pos.clone();
                let workers = workers.clone();
                let update_sender = update_sender.clone();
                let tx = tx.clone();

                s.spawn(move |_| {
                    let worker_id = rand::random::<u64>();
                    let worker_name = format!("MCTS-{}", i);

                    if let Some(sender) = &update_sender {
                        let _ = sender.send(EvolutionUpdate::StatusUpdate(format!("Worker [{:x}] starting: {}", worker_id, worker_name)));
                    }

                    if let Some(w) = &workers {
                        let mut worker_list = w.lock().unwrap();
                        worker_list.push(Worker { id: worker_id, name: worker_name.clone(), start_time: Instant::now() });
                    }

                    let mut local_searcher = MctsSearcher::new();

                    for _ in 0..simulations_per_thread {
                        let (leaf_index, leaf_pos) = local_searcher.select(0, &pos);
                        local_searcher.expand(leaf_index, &leaf_pos);
                        let result = local_searcher.simulate(&leaf_pos);
                        local_searcher.backpropagate(leaf_index, result);
                    }

                    tx.send(local_searcher.tree).unwrap();

                    if let Some(w) = &workers {
                        let mut worker_list = w.lock().unwrap();
                        worker_list.retain(|worker| worker.id != worker_id);
                    }

                    if let Some(sender) = &update_sender {
                        let _ = sender.send(EvolutionUpdate::StatusUpdate(format!("Worker [{:x}] finished.", worker_id)));
                    }
                });
            }
        }).unwrap();

        drop(tx);

        for remote_tree in rx.iter() {
            for (i, node) in remote_tree.iter().enumerate() {
                if i >= self.tree.len() {
                    self.tree.push(node.clone());
                } else {
                    self.tree[i].visits += node.visits;
                    self.tree[i].wins += node.wins;
                    for (m, child_index) in &node.children {
                        if !self.tree[i].children.contains_key(m) {
                            self.tree[i].children.insert(*m, *child_index);
                        }
                    }
                }
            }
        }

        (
            self.best_move(),
            0,
            Some(self.build_move_tree_recursive(0, pos, "root".to_string())),
        )
    }
}