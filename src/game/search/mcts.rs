// src/game/search/mcts.rs

use crate::game::evaluation;
use crate::game::evaluation::see;
use crate::game::search::{MoveTreeNode, SearchConfig, Searcher, MctsCache, MctsNodeData};
use shakmaty::{Chess, Move, Position, EnPassantMode};
use shakmaty::zobrist::ZobristHash;
use std::sync::{Arc, Mutex};
use std::time::Instant;

#[derive(Debug, Clone, Copy, Default)]
pub struct MctsStats {
    pub max_depth: u32,
    pub branches_evaluated: u32,
}

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
}

impl Searcher for MctsSearcher {
    fn search(
        &mut self,
        pos: &Chess,
        _depth: u8,
        config: &SearchConfig,
        _build_tree: bool,
        verbose: bool,
    ) -> (Option<Move>, i32, Option<MoveTreeNode>, Option<String>) {
        let (best_move, score, final_tree, stats) = self.mcts(pos, config, verbose);
        let stats_string = format!(
            "MCTS Stats: Max Depth={}, Branches Evaluated={}",
            stats.max_depth, stats.branches_evaluated
        );
        (best_move, score, Some(final_tree), Some(stats_string))
    }
}

impl MctsSearcher {
    fn mcts(
        &self,
        pos: &Chess,
        config: &SearchConfig,
        verbose: bool,
    ) -> (Option<Move>, i32, MoveTreeNode, MctsStats) {
        if verbose {
            let fen = shakmaty::fen::Fen::from_position(pos, EnPassantMode::Legal);
            println!("MCTS evaluation started for position: {fen}");
        }

        if pos.is_game_over() {
            if verbose {
                println!("MCTS task finished: Game is already over.");
            }
            let score = evaluation::evaluate(pos, config);
            return (
                None,
                score,
                MoveTreeNode {
                    move_san: "root".to_string(),
                    score,
                    children: vec![],
                },
                MctsStats::default(),
            );
        }

        let mut root = Node::new(pos, None, Arc::clone(&self.mcts_cache));
        let mut stats = MctsStats::default();
        let start_time = Instant::now();

        for iteration_count in 0..config.mcts_simulations {
            if verbose && iteration_count % 10000 == 0 {
                let best_child = root.children.iter().max_by(|a, b| a.visits.cmp(&b.visits));
                let best_move_san = best_child
                    .and_then(|c| c.parent_move)
                    .map(|m| shakmaty::san::SanPlus::from_move(pos.clone(), m).to_string())
                    .unwrap_or_else(|| "N/A".to_string());

                println!(
                    "MCTS Progress: Branches={}, Time={}s, Current Best Move={}",
                    stats.branches_evaluated,
                    start_time.elapsed().as_secs(),
                    best_move_san
                );
            }
            let mut path_indices = Vec::new();
            let mut current_pos = pos.clone();

            // Selection
            let mut current_node = &root;
            while !current_node.is_leaf() {
                let best_child_idx = current_node
                    .children
                    .iter()
                    .enumerate()
                    .max_by(|(_, a), (_, b)| {
                        let a_ucb1 = a.ucb1(current_node.visits);
                        let b_ucb1 = b.ucb1(current_node.visits);
                        a_ucb1.partial_cmp(&b_ucb1).unwrap_or(std::cmp::Ordering::Equal)
                    })
                    .map(|(i, _)| i)
                    .unwrap(); // This is safe because we handle the None case in expand
                path_indices.push(best_child_idx);
                current_node = &current_node.children[best_child_idx];
            }

            // Expansion
            let mut leaf_node_mut = &mut root;
            for &idx in &path_indices {
                let child_move = leaf_node_mut.children[idx].parent_move.unwrap();
                current_pos.play_unchecked(child_move);
                leaf_node_mut = &mut leaf_node_mut.children[idx];
            }

            if leaf_node_mut.visits != 0 && !current_pos.is_game_over() {
                leaf_node_mut.expand(&current_pos);
            }

            let (sim_start_pos, _node_to_sim) = if !leaf_node_mut.children.is_empty() {
                let random_child_idx = rand::random::<usize>() % leaf_node_mut.children.len();
                path_indices.push(random_child_idx);
                let child_node = &leaf_node_mut.children[random_child_idx];
                let mut sim_pos = current_pos.clone();
                sim_pos.play_unchecked(child_node.parent_move.unwrap());
                (sim_pos, child_node)
            } else {
                (current_pos, &*leaf_node_mut)
            };

            // Simulation
            let mut sim_pos = sim_start_pos;
            let mut sim_depth = 0;
            while !sim_pos.is_game_over() && sim_depth < 50 {
                let moves = sim_pos.legal_moves();
                if moves.is_empty() {
                    break;
                }
                let m = &moves[rand::random::<usize>() % moves.len()];
                sim_pos.play_unchecked(*m);
                sim_depth += 1;
            }

            let eval_score = evaluation::evaluate(&sim_pos, config);
            let win_prob = 1.0 / (1.0 + (-(eval_score as f64) / 400.0).exp());

            // Backpropagation
            let mut node_to_update = &mut root;
            node_to_update.visits += 1;
            node_to_update.wins += win_prob;
            for &idx in &path_indices {
                node_to_update = &mut node_to_update.children[idx];
                node_to_update.visits += 1;
                node_to_update.wins += win_prob;
            }
            stats.branches_evaluated += 1;
            stats.max_depth = stats.max_depth.max(path_indices.len() as u32);
        }

        let best_child = root
            .children
            .iter()
            .max_by(|a, b| a.visits.cmp(&b.visits));

        if let Some(best_child) = best_child {
            let best_move = best_child.parent_move.unwrap();
            let final_tree = root.to_move_tree_node(pos);
            let score = (best_child.wins / best_child.visits as f64 * 100.0) as i32;
            root.update_cache(pos);

            if verbose {
                let san_move = shakmaty::san::SanPlus::from_move(pos.clone(), best_move);
                println!("MCTS task finished: Best move found: {san_move}");
            }

            (Some(best_move), score, final_tree, stats)
        } else {
            if verbose {
                println!("MCTS task finished: No best move found.");
            }
            (
                None,
                0,
                MoveTreeNode {
                    move_san: "root".to_string(),
                    score: 0,
                    children: vec![],
                },
                stats,
            )
        }
    }
}

#[derive(Debug, Clone)]
struct Node {
    parent_move: Option<Move>,
    visits: u32,
    wins: f64,
    children: Vec<Node>,
    mcts_cache: Arc<Mutex<MctsCache>>,
}

impl Node {
    fn new(pos: &Chess, parent_move: Option<Move>, mcts_cache: Arc<Mutex<MctsCache>>) -> Self {
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

    fn expand(&mut self, pos: &Chess) {
        if !self.children.is_empty() {
            return;
        }
        for m in pos.legal_moves() {
            if m.is_capture() {
                if let Some(from) = m.from() {
                    if see::see(pos.board(), from, m.to()) < 0 {
                        continue; // Prune losing captures
                    }
                }
            }
            let mut new_pos = pos.clone();
            new_pos.play_unchecked(m);
            self.children.push(Node::new(&new_pos, Some(m), Arc::clone(&self.mcts_cache)));
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

    fn to_move_tree_node(&self, parent_pos: &Chess) -> MoveTreeNode {
        let mut current_pos = parent_pos.clone();
        if let Some(m) = self.parent_move {
            current_pos.play_unchecked(m);
        }

        MoveTreeNode {
            move_san: self
                .parent_move
                .map(|m| shakmaty::san::SanPlus::from_move(parent_pos.clone(), m).to_string())
                .unwrap_or_else(|| "root".to_string()),
            score: if self.visits > 0 { (self.wins / self.visits as f64 * 100.0) as i32 } else { 0 },
            children: self
                .children
                .iter()
                .map(|c| c.to_move_tree_node(&current_pos))
                .collect(),
        }
    }

    fn update_cache(&self, pos: &Chess) {
        let mut cache = self.mcts_cache.lock().unwrap();
        let hash = pos.zobrist_hash::<crate::game::search::evaluation_cache::Zobrist64>(EnPassantMode::Legal);
        cache.store(hash, MctsNodeData { visits: self.visits, wins: self.wins });
        for child in &self.children {
            let mut new_pos = pos.clone();
            new_pos.play_unchecked(child.parent_move.unwrap());
            child.update_cache(&new_pos);
        }
    }
}