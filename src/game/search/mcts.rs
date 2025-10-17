// src/game/search/mcts.rs

use crate::game::evaluation;
use crate::game::evaluation::see;
use crate::game::search::{MoveTreeNode, SearchConfig, Searcher, MctsCache, MctsNodeData};
use crossbeam_channel::Sender;
use crossbeam_utils::thread;
use num_cpus;
use shakmaty::{Chess, Move, Position, EnPassantMode};
use shakmaty::zobrist::ZobristHash;
use std::sync::{Arc, Mutex};
use crate::app::Worker;
use crate::event::Event;
use std::any::Any;

pub struct MctsSearcher {
    mcts_cache: Arc<Mutex<MctsCache>>,
}

impl Default for MctsSearcher {
    fn default() -> Self {
        Self::new()
    }
}

impl MctsSearcher {
    pub fn new() -> Self {
        Self {
            mcts_cache: Arc::new(Mutex::new(MctsCache::new())),
        }
    }

    pub fn with_shared_cache(cache: Arc<Mutex<MctsCache>>) -> Self {
        Self { mcts_cache: cache }
    }
}

impl Searcher for MctsSearcher {
    fn search(
        &mut self,
        pos: &Chess,
        _depth: u8,
        config: &SearchConfig,
        _workers: Option<Arc<Mutex<Vec<Worker>>>>,
        _update_sender: Option<Sender<Event>>,
    ) -> (Option<Move>, i32, Option<MoveTreeNode>) {
        let (best_move, score, final_tree) = self.mcts(pos, config.mcts_simulations, config);
        (best_move, score, Some(final_tree))
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl MctsSearcher {
    fn mcts(
        &self,
        pos: &Chess,
        simulations: u32,
        config: &SearchConfig,
    ) -> (Option<Move>, i32, MoveTreeNode) {
        if pos.is_game_over() {
            let score = evaluation::evaluate(pos, config);
            return (
                None,
                score,
                MoveTreeNode {
                    move_san: "root".to_string(),
                    score,
                    children: vec![],
                },
            );
        }

        let root = Node::new(pos.clone(), None, Arc::clone(&self.mcts_cache));
        let root_mutex = Arc::new(Mutex::new(root));

        let num_threads = num_cpus::get();
        let simulations_per_thread = simulations / num_threads as u32;

        thread::scope(|s| {
            for _ in 0..num_threads {
                let root_clone = Arc::clone(&root_mutex);
                let config_clone = config.clone();

                s.spawn(move |_| {
                    for _ in 0..simulations_per_thread {
                        let mut root_guard = root_clone.lock().unwrap();
                        let mut path_indices = Vec::new();

                        // Selection
                        let mut current_node = &*root_guard;
                        while !current_node.is_leaf() {
                             let best_child_idx = current_node
                                .children
                                .iter()
                                .enumerate()
                                .max_by(|(_, a), (_, b)| {
                                    let a_ucb1 = a.ucb1(current_node.visits);
                                    let b_ucb1 = b.ucb1(current_node.visits);
                                    a_ucb1.partial_cmp(&b_ucb1).unwrap()
                                })
                                .map(|(i, _)| i)
                                .unwrap();
                            path_indices.push(best_child_idx);
                            current_node = &current_node.children[best_child_idx];
                        }

                        // Expansion
                        let mut leaf_node = &mut *root_guard;
                        for &idx in &path_indices {
                            leaf_node = &mut leaf_node.children[idx];
                        }

                        if leaf_node.visits > 0 {
                            leaf_node.expand();
                        }

                        let mut node_to_sim = leaf_node;
                        if !node_to_sim.children.is_empty() {
                            let random_child_idx = rand::random::<usize>() % node_to_sim.children.len();
                            path_indices.push(random_child_idx);
                            node_to_sim = &mut node_to_sim.children[random_child_idx];
                        }


                        // Simulation
                        let mut sim_pos = node_to_sim.pos.clone();
                        let mut sim_depth = 0;
                        while !sim_pos.is_game_over() && sim_depth < 10 {
                            let moves = sim_pos.legal_moves();
                            if moves.is_empty() { break; }
                            if let Some(m) = moves.get(rand::random::<usize>() % moves.len()) {
                                sim_pos.play_unchecked(*m);
                            } else {
                                break;
                            }
                            sim_depth += 1;
                        }

                        let eval_score = evaluation::evaluate(&sim_pos, &config_clone);
                        let win_prob = 1.0 / (1.0 + (-(eval_score as f64) / 400.0).exp());

                        // Backpropagation
                        let mut node_to_update = &mut *root_guard;
                        node_to_update.visits += 1;
                        node_to_update.wins += win_prob;
                        for &idx in &path_indices {
                            node_to_update = &mut node_to_update.children[idx];
                            node_to_update.visits += 1;
                            node_to_update.wins += win_prob;
                        }
                    }
                });
            }
        })
        .unwrap();

        let root = Arc::try_unwrap(root_mutex).unwrap().into_inner().unwrap();

        let best_child = root
            .children
            .iter()
            .max_by(|a, b| a.visits.cmp(&b.visits));

        if best_child.is_none() {
             return (
                None,
                0,
                MoveTreeNode {
                    move_san: "root".to_string(),
                    score: 0,
                    children: vec![],
                },
            );
        }
        let best_child = best_child.unwrap();


        let best_move = best_child.parent_move.unwrap();
        let final_tree = root.to_move_tree_node();
        let score = (best_child.wins / best_child.visits as f64 * 100.0) as i32;

        root.update_cache();

        (Some(best_move), score, final_tree)
    }
}

#[derive(Debug, Clone)]
struct Node {
    pos: Chess,
    parent_move: Option<Move>,
    visits: u32,
    wins: f64,
    children: Vec<Node>,
    mcts_cache: Arc<Mutex<MctsCache>>,
}

impl Node {
    fn new(pos: Chess, parent_move: Option<Move>, mcts_cache: Arc<Mutex<MctsCache>>) -> Self {
        let hash = pos.zobrist_hash::<crate::game::search::evaluation_cache::Zobrist64>(EnPassantMode::Legal);
        let (visits, wins) = {
            let cache = mcts_cache.lock().unwrap();
            if let Some(data) = cache.probe(&hash) {
                (data.visits, data.wins)
            } else {
                (0, 0.0)
            }
        };

        Self {
            pos,
            parent_move,
            visits,
            wins,
            children: Vec::new(),
            mcts_cache,
        }
    }

    fn is_leaf(&self) -> bool {
        self.children.is_empty()
    }

    fn expand(&mut self) {
        if !self.children.is_empty() {
            return;
        }
        for m in self.pos.legal_moves() {
            if m.is_capture() {
                if let Some(from) = m.from() {
                    if see::see(self.pos.board(), from, m.to()) < 0 {
                        continue; // Prune losing captures
                    }
                }
            }
            let mut new_pos = self.pos.clone();
            new_pos.play_unchecked(m);
            self.children.push(Node::new(new_pos, Some(m), Arc::clone(&self.mcts_cache)));
        }
    }

    fn ucb1(&self, parent_visits: u32) -> f64 {
        if self.visits == 0 {
            f64::INFINITY
        } else {
            (self.wins / self.visits as f64)
                + (2.0f64.ln() * parent_visits as f64 / self.visits as f64).sqrt()
        }
    }

    fn to_move_tree_node(&self) -> MoveTreeNode {
        MoveTreeNode {
            move_san: self
                .parent_move
                .map(|m| shakmaty::san::SanPlus::from_move(self.pos.clone(), m).to_string())
                .unwrap_or_else(|| "root".to_string()),
            score: if self.visits > 0 { (self.wins / self.visits as f64 * 100.0) as i32 } else { 0 },
            children: self
                .children
                .iter()
                .map(|c| c.to_move_tree_node())
                .collect(),
        }
    }

    fn update_cache(&self) {
        let mut cache = self.mcts_cache.lock().unwrap();
        let hash = self.pos.zobrist_hash::<crate::game::search::evaluation_cache::Zobrist64>(EnPassantMode::Legal);
        cache.store(hash, MctsNodeData { visits: self.visits, wins: self.wins });
        for child in &self.children {
            child.update_cache();
        }
    }
}