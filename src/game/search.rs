// src/game/search.rs

pub mod mcts;
pub mod evaluation_cache;
pub mod mcts_cache;

use shakmaty::{Chess, Move, Position, Piece, san::SanPlus, EnPassantMode};
use shakmaty::zobrist::ZobristHash;
use crate::game::evaluation;
use crossbeam_utils::thread;
use num_cpus;
use evaluation_cache::EvaluationCache;
pub use mcts_cache::{MctsCache, MctsNodeData};
use crate::constants::MATE_SCORE;

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum SearchAlgorithm {
    Pvs,
    Mcts,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SearchConfig {
    pub search_depth: u8,
    pub search_algorithm: SearchAlgorithm,
    pub use_aspiration_windows: bool,
    pub use_history_heuristic: bool,
    pub use_killer_moves: bool,
    pub mcts_simulations: u32,
    pub use_quiescence_search: bool,
    pub use_pvs: bool,
    pub use_null_move_pruning: bool,
    pub use_lmr: bool,
    pub use_futility_pruning: bool,
    pub use_delta_pruning: bool,
    pub pawn_structure_weight: i32,
    pub piece_mobility_weight: i32,
    pub king_safety_weight: i32,
    pub piece_development_weight: i32,
    pub rook_placement_weight: i32,
    pub bishop_placement_weight: i32,
    pub knight_placement_weight: i32,
    pub passed_pawn_weight: i32,
    pub isolated_pawn_weight: i32,
    pub doubled_pawn_weight: i32,
    pub bishop_pair_weight: i32,
    pub pawn_chain_weight: i32,
    pub ram_weight: i32,
    pub candidate_passed_pawn_weight: i32,
    pub king_pawn_shield_weight: i32,
    pub king_open_file_penalty: i32,
    pub king_attackers_weight: i32,
    pub threat_analysis_weight: i32,
    pub tempo_bonus_weight: i32,
    pub space_evaluation_weight: i32,
    pub initiative_evaluation_weight: i32,
}

impl SearchConfig {
    pub fn default_with_randomization(rng: &mut impl rand::Rng) -> Self {
        let mut config = Self::default();
        let default_config = Self::default(); // for reference values

        config.search_depth = rng.gen_range(3..=5);

        // Randomize booleans
        config.use_aspiration_windows = rng.gen_bool(0.5);
        config.use_history_heuristic = rng.gen_bool(0.5);
        config.use_killer_moves = rng.gen_bool(0.5);
        config.use_quiescence_search = rng.gen_bool(0.5);
        config.use_pvs = rng.gen_bool(0.5);
        config.use_null_move_pruning = rng.gen_bool(0.5);
        config.use_lmr = rng.gen_bool(0.5);
        config.use_futility_pruning = rng.gen_bool(0.5);
        config.use_delta_pruning = rng.gen_bool(0.5);

        // Randomize enum
        config.search_algorithm = SearchAlgorithm::Pvs;

        // Helper function for numeric randomization
        let mut vary_numeric = |value: i32| -> i32 {
            let factor = rng.gen_range(-0.5..=0.5);
            (value as f64 * (1.0 + factor)).round() as i32
        };

        // Randomize numeric values with +/- 50% variance
        config.mcts_simulations = vary_numeric(default_config.mcts_simulations as i32) as u32;
        config.pawn_structure_weight = vary_numeric(default_config.pawn_structure_weight);
        config.piece_mobility_weight = vary_numeric(default_config.piece_mobility_weight);
        config.king_safety_weight = vary_numeric(default_config.king_safety_weight);
        config.piece_development_weight = vary_numeric(default_config.piece_development_weight);
        config.rook_placement_weight = vary_numeric(default_config.rook_placement_weight);
        config.bishop_placement_weight = vary_numeric(default_config.bishop_placement_weight);
        config.knight_placement_weight = vary_numeric(default_config.knight_placement_weight);
        config.passed_pawn_weight = vary_numeric(default_config.passed_pawn_weight);
        config.isolated_pawn_weight = vary_numeric(default_config.isolated_pawn_weight);
        config.doubled_pawn_weight = vary_numeric(default_config.doubled_pawn_weight);
        config.bishop_pair_weight = vary_numeric(default_config.bishop_pair_weight);
        config.pawn_chain_weight = vary_numeric(default_config.pawn_chain_weight);
        config.ram_weight = vary_numeric(default_config.ram_weight);
        config.candidate_passed_pawn_weight = vary_numeric(default_config.candidate_passed_pawn_weight);
        config.king_pawn_shield_weight = vary_numeric(default_config.king_pawn_shield_weight);
        config.king_open_file_penalty = vary_numeric(default_config.king_open_file_penalty);
        config.king_attackers_weight = vary_numeric(default_config.king_attackers_weight);
        config.threat_analysis_weight = vary_numeric(default_config.threat_analysis_weight);
        config.tempo_bonus_weight = vary_numeric(default_config.tempo_bonus_weight);
        config.space_evaluation_weight = vary_numeric(default_config.space_evaluation_weight);
        config.initiative_evaluation_weight = vary_numeric(default_config.initiative_evaluation_weight);

        config
    }
}

impl Default for SearchConfig {
    fn default() -> Self {
        Self {
            search_depth: 5,
            search_algorithm: SearchAlgorithm::Pvs,
            use_aspiration_windows: false,
            use_history_heuristic: false,
            use_killer_moves: false,
            mcts_simulations: 1000,
            use_quiescence_search: true,
            use_pvs: true, // Default to true now that it's properly implemented
            use_null_move_pruning: true,
            use_lmr: true,
            use_futility_pruning: true,
            use_delta_pruning: false,
            pawn_structure_weight: 100,
            piece_mobility_weight: 100,
            king_safety_weight: 100,
            piece_development_weight: 100,
            rook_placement_weight: 100,
            bishop_placement_weight: 100,
            knight_placement_weight: 100,
            passed_pawn_weight: 100,
            isolated_pawn_weight: 100,
            doubled_pawn_weight: 100,
            bishop_pair_weight: 100,
            pawn_chain_weight: 100,
            ram_weight: 100,
            candidate_passed_pawn_weight: 100,
            king_pawn_shield_weight: 100,
            king_open_file_penalty: 100,
            king_attackers_weight: 100,
            threat_analysis_weight: 100,
            tempo_bonus_weight: 10,
            space_evaluation_weight: 100,
            initiative_evaluation_weight: 100,
        }
    }
}

use std::sync::{Arc, Mutex};
use std::time::Instant;
use crate::app::Worker;

#[derive(Clone, Debug)]
pub struct MoveTreeNode {
    pub move_san: String,
    pub score: i32,
    pub children: Vec<MoveTreeNode>,
}

pub trait Searcher: Send {
    fn search(
        &mut self,
        pos: &Chess,
        depth: u8,
        config: &SearchConfig,
        workers: Option<Arc<Mutex<Vec<Worker>>>>,
        match_id: Option<usize>,
    ) -> (Option<Move>, i32, Option<MoveTreeNode>);
}

#[derive(Clone)]
pub struct PvsSearcher {
    history_table: [[i32; 64]; 12],
    killer_moves: [[Option<Move>; 2]; 64],
    evaluation_cache: Arc<Mutex<EvaluationCache>>,
}

impl Searcher for PvsSearcher {
    fn search(
        &mut self,
        pos: &Chess,
        depth: u8,
        config: &SearchConfig,
        workers: Option<Arc<Mutex<Vec<Worker>>>>,
        match_id: Option<usize>,
    ) -> (Option<Move>, i32, Option<MoveTreeNode>) {
        if !config.use_aspiration_windows {
            let args = PvsRootSearchArgs {
                pos, depth, config, alpha: -MATE_SCORE, beta: MATE_SCORE, workers, match_id
            };
            let (move_opt, score, tree) = self.pvs_root_search(args);
            return (move_opt, score, Some(tree));
        }

        const ASPIRATION_WINDOW_DELTA: i32 = 50;
        let score_guess = self.evaluate_with_cache(pos, config);
        let alpha = score_guess - ASPIRATION_WINDOW_DELTA;
        let beta = score_guess + ASPIRATION_WINDOW_DELTA;

        let args = PvsRootSearchArgs {
            pos, depth, config, alpha, beta, workers: workers.clone(), match_id
        };
        let (mut best_move, mut score, mut tree) = self.pvs_root_search(args);

        if score <= alpha || score >= beta {
            let args = PvsRootSearchArgs {
                pos, depth, config, alpha: -MATE_SCORE, beta: MATE_SCORE, workers, match_id
            };
            (best_move, score, tree) = self.pvs_root_search(args);
        }

        (best_move, score, Some(tree))
    }
}

struct PvsRootSearchArgs<'a> {
    pos: &'a Chess,
    depth: u8,
    config: &'a SearchConfig,
    alpha: i32,
    beta: i32,
    workers: Option<Arc<Mutex<Vec<Worker>>>>,
    match_id: Option<usize>,
}

impl PvsSearcher {
    pub fn with_shared_cache(cache: Arc<Mutex<EvaluationCache>>) -> Self {
        Self {
            history_table: [[0; 64]; 12],
            killer_moves: [[None; 2]; 64],
            evaluation_cache: cache,
        }
    }

    fn pvs_root_search(
        &mut self,
        mut args: PvsRootSearchArgs,
    ) -> (Option<Move>, i32, MoveTreeNode) {
        let mut legal_moves = args.pos.legal_moves();
        let mut root_node = MoveTreeNode {
            move_san: "root".to_string(),
            score: 0,
            children: Vec::new(),
        };
        if legal_moves.is_empty() {
            return (None, self.evaluate_with_cache(args.pos, args.config), root_node);
        }
        self.order_moves(&mut legal_moves, args.pos, 0, args.config, None);

        let num_threads = num_cpus::get();
        let (tx, rx) = std::sync::mpsc::channel();

        thread::scope(|s| {
            for moves_chunk in legal_moves.chunks( (legal_moves.len() / num_threads).max(1) ) {
                let pos = args.pos.clone();
                let config = args.config.clone();
                let mut searcher = self.clone();
                let tx = tx.clone();
                let workers = args.workers.clone();
                let match_id = args.match_id;
                let moves_chunk_owned: Vec<Move> = moves_chunk.to_vec();

                s.spawn(move |_| {
                    let worker_id = rand::random::<u64>();
                    let worker_name = moves_chunk_owned.iter()
                        .map(|m| SanPlus::from_move(pos.clone(), *m).to_string())
                        .collect::<Vec<_>>()
                        .join(", ");

                    let worker_display_name = if let Some(id) = match_id {
                        format!("M {id} {worker_name}")
                    } else {
                        worker_name.to_string()
                    };

                    if let Some(w) = &workers {
                        let mut worker_list = w.lock().unwrap();
                        worker_list.push(Worker {
                            id: worker_id,
                            name: worker_display_name.clone(),
                            start_time: Instant::now(),
                        });
                    }

                    let mut chunk_alpha = -MATE_SCORE;

                    for m in &moves_chunk_owned {
                        let mut new_pos = pos.clone();
                        new_pos.play_unchecked(*m);
                        let (score, child_node) =
                            searcher.alpha_beta(&new_pos, args.depth - 1, 1, -args.beta, -chunk_alpha, &config);
                        let score = -score;

                        if score > chunk_alpha {
                            chunk_alpha = score;
                        }

                        let san = SanPlus::from_move(pos.clone(), *m);
                        let mut node = child_node;
                        node.move_san = san.to_string();
                        node.score = score;
                        tx.send(((Some(*m), score), node)).unwrap();
                    }

                    if let Some(w) = &workers {
                        let mut worker_list = w.lock().unwrap();
                        worker_list.retain(|worker| worker.id != worker_id);
                    }
                });
            }
        })
        .unwrap();

        drop(tx);

        let mut best_move = None;
        for ((move_option, score), node) in rx.iter() {
            root_node.children.push(node);
            if score > args.alpha {
                args.alpha = score;
                best_move = move_option;
            }
        }

        root_node.score = args.alpha;
        (best_move, args.alpha, root_node)
    }

    fn alpha_beta(
        &mut self,
        pos: &Chess,
        depth: u8,
        ply: u8,
        alpha: i32,
        beta: i32,
        config: &SearchConfig,
    ) -> (i32, MoveTreeNode) {
        if config.use_null_move_pruning {
            return self.null_move_search(pos, depth, ply, alpha, beta, config);
        }
        self.pvs_search(pos, depth, ply, alpha, beta, config)
    }

    fn null_move_search(
        &mut self,
        pos: &Chess,
        depth: u8,
        ply: u8,
        alpha: i32,
        beta: i32,
        config: &SearchConfig,
    ) -> (i32, MoveTreeNode) {
        const NMP_DEPTH_REDUCTION: u8 = 3;
        const NMP_MIN_DEPTH: u8 = 3;

        let non_pawn_material = pos.board().by_color(pos.turn()) & !pos.board().pawns();
        let is_likely_zugzwang = non_pawn_material.is_empty();

        if depth >= NMP_MIN_DEPTH && !pos.is_check() && !is_likely_zugzwang {
            if let Ok(null_move_pos) = pos.clone().swap_turn() {
                let (score, _) = self.pvs_search(
                    &null_move_pos,
                    depth.saturating_sub(NMP_DEPTH_REDUCTION),
                    ply + 1,
                    -beta,
                    -beta + 1,
                    config,
                );
                let score = -score;
                if score >= beta {
                    return (beta, MoveTreeNode { move_san: "null".to_string(), score: beta, children: vec![] });
                }
            }
        }
        self.pvs_search(pos, depth, ply, alpha, beta, config)
    }

    fn pvs_search(
        &mut self,
        pos: &Chess,
        depth: u8,
        ply: u8,
        alpha: i32,
        beta: i32,
        config: &SearchConfig,
    ) -> (i32, MoveTreeNode) {
        const LMR_MIN_DEPTH: u8 = 3;
        const LMR_MIN_MOVE_INDEX: usize = 2;
        const FUTILITY_MARGIN_PER_DEPTH: [i32; 4] = [0, 100, 250, 500];

        let mut current_node = MoveTreeNode {
            move_san: "".to_string(), // This will be set by the parent
            score: alpha,
            children: Vec::new(),
        };

        if pos.is_game_over() && pos.outcome().winner().is_none() {
            return (0, current_node);
        }

        let mut legal_moves = pos.legal_moves();
        if legal_moves.is_empty() {
            if pos.is_checkmate() { return (-MATE_SCORE + ply as i32, current_node); }
            return (0, current_node); // Stalemate
        }

        if depth == 0 {
            let score = if config.use_quiescence_search {
                self.quiescence_search(pos, alpha, beta, config)
            } else {
                self.evaluate_with_cache(pos, config)
            };
            current_node.score = score;
            return (score, current_node);
        }

        if config.use_futility_pruning && (depth as usize) < FUTILITY_MARGIN_PER_DEPTH.len() {
            let eval = self.evaluate_with_cache(pos, config);
            let margin = FUTILITY_MARGIN_PER_DEPTH[depth as usize];
            if eval + margin <= alpha {
                let score = self.quiescence_search(pos, alpha, beta, config);
                current_node.score = score;
                return (score, current_node)
            }
        }

        self.order_moves(&mut legal_moves, pos, ply, config, None);

        let mut a = alpha;

        for (i, m) in legal_moves.into_iter().enumerate() {
            let mut new_pos = pos.clone();
            new_pos.play_unchecked(m);

            let (score, child_node) = if i == 0 {
                let (s, cn) =
                    self.pvs_search(&new_pos, depth - 1, ply + 1, -beta, -a, config);
                (-s, cn)
            } else {
                let mut reduction = 0;
                if config.use_lmr
                    && depth >= LMR_MIN_DEPTH
                    && i >= LMR_MIN_MOVE_INDEX
                    && !pos.is_check()
                    && !m.is_capture()
                {
                    reduction = (1.0 + (depth as f32).ln() * (i as f32).ln() / 2.0).floor() as u8;
                    reduction = reduction.min(depth - 1);
                }

                let (zw_score, child_node) = self.pvs_search(
                    &new_pos,
                    depth - 1 - reduction,
                    ply + 1,
                    -a - 1,
                    -a,
                    config,
                );
                let zw_score = -zw_score;

                if zw_score > a && zw_score < beta {
                    let (s, cn) =
                        self.pvs_search(&new_pos, depth - 1, ply + 1, -beta, -a, config);
                    (-s, cn)
                } else {
                    (zw_score, child_node)
                }
            };

            let san = SanPlus::from_move(pos.clone(), m);
            let mut new_node = child_node;
            new_node.move_san = san.to_string();
            new_node.score = score;
            current_node.children.push(new_node);

            if score >= beta {
                if config.use_killer_moves && !m.is_capture() {
                    self.killer_moves[ply as usize][1] = self.killer_moves[ply as usize][0];
                    self.killer_moves[ply as usize][0] = Some(m);
                }
                current_node.score = beta;
                return (beta, current_node);
            }
            if score > a {
                a = score;
                if config.use_history_heuristic {
                    if let Some(from_sq) = m.from() {
                        let piece_index = self.get_piece_index(pos.board().piece_at(from_sq).unwrap());
                        self.history_table[piece_index][m.to() as usize] += (depth as i32).pow(2);
                    }
                }
            }
        }

        current_node.score = a;
        (a, current_node)
    }

    fn get_piece_index(&self, piece: Piece) -> usize {
        // Shakmaty's Role enum appears to be 1-indexed when cast to usize,
        // with Pawn = 1, King = 6. We subtract 1 to get a 0-based index.
        (piece.color as usize * 6) + (piece.role as usize - 1)
    }

    fn order_moves(&self, moves: &mut [Move], pos: &Chess, ply: u8, config: &SearchConfig, tt_move: Option<Move>) {
        moves.sort_unstable_by(|a, b| {
            let a_score = self.score_move(a, pos, ply, config, tt_move);
            let b_score = self.score_move(b, pos, ply, config, tt_move);
            b_score.cmp(&a_score)
        });
    }

    fn score_move(&self, m: &Move, pos: &Chess, ply: u8, config: &SearchConfig, _: Option<Move>) -> i32 {
        if m.is_capture() {
            return 1_000_000; // High score for captures to search them first
        }
        if config.use_killer_moves {
            if Some(*m) == self.killer_moves[ply as usize][0] {
                return 900_000;
            }
            if Some(*m) == self.killer_moves[ply as usize][1] {
                return 800_000;
            }
        }
        if config.use_history_heuristic {
            if let Some(from_sq) = m.from() {
                let piece_index = self.get_piece_index(pos.board().piece_at(from_sq).unwrap());
                return self.history_table[piece_index][m.to() as usize];
            }
        }
        0
    }

    fn evaluate_with_cache(&self, pos: &Chess, config: &SearchConfig) -> i32 {
        let hash = pos.zobrist_hash::<evaluation_cache::Zobrist64>(EnPassantMode::Legal);
        if let Some(score) = self.evaluation_cache.lock().unwrap().probe(&hash) {
            return score;
        }

        let score = evaluation::evaluate(pos, config);
        self.evaluation_cache.lock().unwrap().store(evaluation_cache::CacheEntry { hash, score });
        score
    }

    fn quiescence_search(&self, pos: &Chess, mut alpha: i32, beta: i32, config: &SearchConfig) -> i32 {
        if config.use_delta_pruning {
            return self.delta_search(pos, alpha, beta, config);
        }

        let standing_pat = self.evaluate_with_cache(pos, config);
        if standing_pat >= beta {
            return beta;
        }
        if alpha < standing_pat {
            alpha = standing_pat;
        }

        let captures = pos.legal_moves().into_iter().filter(|m| m.is_capture());

        for m in captures {
            if evaluation::see::see(pos.board(), m.from().unwrap(), m.to()) < 0 {
                continue;
            }
            let mut new_pos = pos.clone();
            new_pos.play_unchecked(m);

            if new_pos.legal_moves().is_empty() {
                if new_pos.is_checkmate() {
                    return MATE_SCORE - 100;
                }
                return 0;
            }

            let score = -self.quiescence_search(&new_pos, -beta, -alpha, config);

            if score >= beta {
                return beta;
            }
            if score > alpha {
                alpha = score;
            }
        }
        alpha
    }

    fn delta_search(&self, pos: &Chess, mut alpha: i32, beta: i32, config: &SearchConfig) -> i32 {
        let standing_pat = self.evaluate_with_cache(pos, config);
        if standing_pat >= beta {
            return beta;
        }
        if alpha < standing_pat {
            alpha = standing_pat;
        }

        if config.use_delta_pruning {
            let queen_value = evaluation::get_piece_value(shakmaty::Role::Queen);
            if standing_pat - queen_value >= beta {
                return beta;
            }
        }

        let captures = pos.legal_moves().into_iter().filter(|m| m.is_capture());

        for m in captures {
            if evaluation::see::see(pos.board(), m.from().unwrap(), m.to()) < 0 {
                continue;
            }

            let mut new_pos = pos.clone();
            new_pos.play_unchecked(m);
            let score = -self.delta_search(&new_pos, -beta, -alpha, config);

            if score >= beta {
                return beta;
            }
            if score > alpha {
                alpha = score;
            }
        }

        alpha
    }
}