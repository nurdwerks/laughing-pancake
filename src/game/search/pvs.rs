// src/game/search/pvs.rs

//! Principal Variation Search (PVS)
//!
//! PVS is an optimization of the alpha-beta algorithm that improves search
//! efficiency. It operates on the assumption that the first move checked will
//! likely be the best one. This move is searched with a full alpha-beta window.
//! All subsequent moves are then searched with a narrower "zero window"
//! (where beta = alpha + 1) to quickly prove they are inferior. If a move
//! searched with a zero window is found to be better than alpha, it is then
//! re-searched with the full window to get a more accurate score. This module
//! also integrates Late Move Reductions (LMR) and Futility Pruning for
//! additional search optimizations.

use shakmaty::{Chess, Position};

use crate::game::evaluation;
use super::{quiescence, MATE_SCORE, SearchConfig};

// Constants for LMR
const LMR_MIN_DEPTH: u8 = 3;
const LMR_MIN_MOVE_INDEX: usize = 2;
const FUTILITY_MARGIN_PER_DEPTH: [i32; 4] = [0, 100, 250, 500];

pub fn search(pos: &Chess, depth: u8, ply: u8, mut alpha: i32, beta: i32, config: &SearchConfig) -> i32 {
    let legal_moves = pos.legal_moves();
    if legal_moves.is_empty() {
        if pos.is_checkmate() {
            return -MATE_SCORE + ply as i32;
        }
        return 0; // Stalemate
    }

    if depth == 0 {
        if config.use_quiescence_search {
            return quiescence::search(pos, alpha, beta, config);
        }
        return evaluation::evaluate(pos, config);
    }

    // --- Futility Pruning at the leaves ---
    if config.use_futility_pruning && (depth as usize) < FUTILITY_MARGIN_PER_DEPTH.len() {
        let eval = evaluation::evaluate(pos, config);
        let margin = FUTILITY_MARGIN_PER_DEPTH[depth as usize];
        if eval + margin <= alpha {
             return quiescence::search(pos, alpha, beta, config);
        }
    }

    for (i, m) in legal_moves.into_iter().enumerate() {
        let mut new_pos = pos.clone();
        new_pos.play_unchecked(m);

        let mut score;
        let is_first_move = i == 0;

        if is_first_move {
            score = -search(&new_pos, depth - 1, ply + 1, -beta, -alpha, config);
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

            score = -search(&new_pos, depth - 1 - reduction, ply + 1, -alpha - 1, -alpha, config);

            if score > alpha && reduction > 0 {
                score = -search(&new_pos, depth - 1, ply + 1, -alpha - 1, -alpha, config);
            }

            if score > alpha && score < beta {
                score = -search(&new_pos, depth - 1, ply + 1, -beta, -alpha, config);
            }
        }

        if score >= beta {
            return beta;
        }
        if score > alpha {
            alpha = score;
        }
    }

    alpha
}
