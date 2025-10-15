// src/game/search.rs

pub mod quiescence;
pub mod mcts;
pub mod delta; // Still needed for quiescence search

use shakmaty::{Chess, Move, Position, Piece};
use crate::game::evaluation;
use crossbeam_utils::thread;
use num_cpus;

const MATE_SCORE: i32 = 1_000_000;

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum SearchAlgorithm {
    Pvs,
    Mcts,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SearchConfig {
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
}

impl Default for SearchConfig {
    fn default() -> Self {
        Self {
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
        }
    }
}

pub trait Searcher {
    fn search(&mut self, pos: &Chess, depth: u8, config: &SearchConfig) -> (Option<Move>, i32);
}

#[derive(Clone)]
pub struct PvsSearcher {
    history_table: [[i32; 64]; 12],
    killer_moves: [[Option<Move>; 2]; 64],
}

impl Searcher for PvsSearcher {
    fn search(&mut self, pos: &Chess, depth: u8, config: &SearchConfig) -> (Option<Move>, i32) {
        if !config.use_aspiration_windows {
            return self.pvs_root_search(pos, depth, config, -MATE_SCORE, MATE_SCORE);
        }

        const ASPIRATION_WINDOW_DELTA: i32 = 50;
        let score_guess = evaluation::evaluate(pos, config);
        let alpha = score_guess - ASPIRATION_WINDOW_DELTA;
        let beta = score_guess + ASPIRATION_WINDOW_DELTA;

        let (mut best_move, mut score) = self.pvs_root_search(pos, depth, config, alpha, beta);

        if score <= alpha || score >= beta {
            (best_move, score) = self.pvs_root_search(pos, depth, config, -MATE_SCORE, MATE_SCORE);
        }

        (best_move, score)
    }
}

impl PvsSearcher {
    fn pvs_root_search(
        &mut self,
        pos: &Chess,
        depth: u8,
        config: &SearchConfig,
        mut alpha: i32,
        beta: i32,
    ) -> (Option<Move>, i32) {
        let mut legal_moves = pos.legal_moves();
        if legal_moves.is_empty() {
            return (None, evaluation::evaluate(pos, config));
        }
        self.order_moves(&mut legal_moves, pos, 0, config);

        let num_threads = num_cpus::get();
        let (tx, rx) = std::sync::mpsc::channel();

        thread::scope(|s| {
            for moves_chunk in legal_moves.chunks( (legal_moves.len() / num_threads).max(1) ) {
                let pos = pos.clone();
                let config = config.clone();
                let mut searcher = self.clone();
                let tx = tx.clone();

                s.spawn(move |_| {
                    let mut chunk_best_move = None;
                    let mut chunk_alpha = -MATE_SCORE;

                    for m in moves_chunk {
                        let mut new_pos = pos.clone();
                        new_pos.play_unchecked(*m);
                        let score = -searcher.alpha_beta(&mut new_pos, depth - 1, 1, -beta, -chunk_alpha, &config);

                        if score > chunk_alpha {
                            chunk_alpha = score;
                            chunk_best_move = Some(*m);
                        }
                    }
                    if let Some(bm) = chunk_best_move {
                        tx.send((Some(bm), chunk_alpha)).unwrap();
                    }
                });
            }
        }).unwrap();

        drop(tx);

        let mut best_move = None;
        for (move_option, score) in rx.iter() {
            if score > alpha {
                alpha = score;
                best_move = move_option;
            }
        }

        (best_move, alpha)
    }

    fn alpha_beta(&mut self, pos: &Chess, depth: u8, ply: u8, alpha: i32, beta: i32, config: &SearchConfig) -> i32 {
        if config.use_null_move_pruning {
            return self.null_move_search(pos, depth, ply, alpha, beta, config);
        }
        self.pvs_search(pos, depth, ply, alpha, beta, config)
    }

    fn null_move_search(&mut self, pos: &Chess, depth: u8, ply: u8, alpha: i32, beta: i32, config: &SearchConfig) -> i32 {
        const NMP_DEPTH_REDUCTION: u8 = 3;
        const NMP_MIN_DEPTH: u8 = 3;

        let non_pawn_material = pos.board().by_color(pos.turn()) & !pos.board().pawns();
        let is_likely_zugzwang = non_pawn_material.is_empty();

        if depth >= NMP_MIN_DEPTH && !pos.is_check() && !is_likely_zugzwang {
            if let Ok(null_move_pos) = pos.clone().swap_turn() {
                let score = -self.pvs_search(&null_move_pos, depth.saturating_sub(NMP_DEPTH_REDUCTION), ply + 1, -beta, -beta + 1, config);
                if score >= beta {
                    return beta;
                }
            }
        }
        self.pvs_search(pos, depth, ply, alpha, beta, config)
    }

    fn pvs_search(&mut self, pos: &Chess, depth: u8, ply: u8, mut alpha: i32, beta: i32, config: &SearchConfig) -> i32 {
        const LMR_MIN_DEPTH: u8 = 3;
        const LMR_MIN_MOVE_INDEX: usize = 2;
        const FUTILITY_MARGIN_PER_DEPTH: [i32; 4] = [0, 100, 250, 500];

        let mut legal_moves = pos.legal_moves();
        if legal_moves.is_empty() {
            if pos.is_checkmate() { return -MATE_SCORE + ply as i32; }
            return 0; // Stalemate
        }

        if depth == 0 {
            if config.use_quiescence_search {
                return quiescence::search(pos, alpha, beta, config);
            }
            return evaluation::evaluate(pos, config);
        }

        if config.use_futility_pruning && (depth as usize) < FUTILITY_MARGIN_PER_DEPTH.len() {
            let eval = evaluation::evaluate(pos, config);
            let margin = FUTILITY_MARGIN_PER_DEPTH[depth as usize];
            if eval + margin <= alpha {
                return quiescence::search(pos, alpha, beta, config);
            }
        }

        self.order_moves(&mut legal_moves, pos, ply, config);

        for (i, m) in legal_moves.into_iter().enumerate() {
            let mut new_pos = pos.clone();
            new_pos.play_unchecked(m);

            let score = if i == 0 {
                -self.alpha_beta(&mut new_pos, depth - 1, ply + 1, -beta, -alpha, config)
            } else {
                let mut reduction = 0;
                if config.use_lmr && depth >= LMR_MIN_DEPTH && i >= LMR_MIN_MOVE_INDEX && !pos.is_check() && !m.is_capture() {
                    reduction = (1.0 + (depth as f32).ln() * (i as f32).ln() / 2.0).floor() as u8;
                    reduction = reduction.min(depth - 1);
                }

                let zw_score = -self.alpha_beta(&mut new_pos, depth - 1 - reduction, ply + 1, -alpha - 1, -alpha, config);

                if zw_score > alpha && reduction > 0 {
                    -self.alpha_beta(&mut new_pos, depth - 1, ply + 1, -alpha - 1, -alpha, config)
                } else if zw_score > alpha && zw_score < beta {
                    -self.alpha_beta(&mut new_pos, depth - 1, ply + 1, -beta, -alpha, config)
                } else {
                    zw_score
                }
            };

            if score >= beta {
                if config.use_killer_moves && !m.is_capture() {
                    self.killer_moves[ply as usize][1] = self.killer_moves[ply as usize][0];
                    self.killer_moves[ply as usize][0] = Some(m);
                }
                return beta;
            }
            if score > alpha {
                alpha = score;
                if config.use_history_heuristic {
                    if let Some(from_sq) = m.from() {
                        let piece_index = self.get_piece_index(pos.board().piece_at(from_sq).unwrap());
                        self.history_table[piece_index][m.to() as usize] += (depth as i32).pow(2);
                    }
                }
            }
        }
        alpha
    }

    fn get_piece_index(&self, piece: Piece) -> usize {
        (piece.color as usize * 6) + piece.role as usize
    }

    fn order_moves(&self, moves: &mut [Move], pos: &Chess, ply: u8, config: &SearchConfig) {
        moves.sort_unstable_by(|a, b| {
            let a_score = self.score_move(a, pos, ply, config);
            let b_score = self.score_move(b, pos, ply, config);
            b_score.cmp(&a_score)
        });
    }

    fn score_move(&self, m: &Move, pos: &Chess, ply: u8, config: &SearchConfig) -> i32 {
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
}

use mcts::MctsSearcher;
pub fn search(pos: &Chess, depth: u8, config: &SearchConfig) -> (Option<Move>, i32) {
    match config.search_algorithm {
        SearchAlgorithm::Pvs => {
            let mut searcher = PvsSearcher {
                history_table: [[0; 64]; 12],
                killer_moves: [[None; 2]; 64],
            };
            searcher.search(pos, depth, config)
        }
        SearchAlgorithm::Mcts => {
            let mut searcher = MctsSearcher::new();
            searcher.search(pos, depth, config)
        }
    }
}
