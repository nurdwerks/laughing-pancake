// src/game/search/pvs.rs

//! Principal Variation Search (PVS)
//!
//! This is a stub for the PVS algorithm. PVS is an optimization of alpha-beta
//! search that can be more efficient in practice. It assumes that the first
//! move checked is the best one and searches it with a full alpha-beta window.
//! Subsequent moves are then searched with a "zero window" (alpha = beta - 1)
//! to prove that they are worse than the first move. If a move is found to be
//! better, it is re-searched with a full window.

use shakmaty::{Chess, Position};

use crate::game::evaluation;
use super::{quiescence, MATE_SCORE, SearchConfig};


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
            return quiescence::search(pos, alpha, beta);
        }
        return evaluation::evaluate(pos);
    }

    let mut is_first_move = true;

    for m in legal_moves {
        let mut new_pos = pos.clone();
        new_pos.play_unchecked(m);

        let score = if is_first_move {
            is_first_move = false;
            -search(&new_pos, depth - 1, ply + 1, -beta, -alpha, config)
        } else {
            // Null-window search
            let mut score = -search(&new_pos, depth - 1, ply + 1, -alpha -1, -alpha, config);

            // If the null-window search fails high, re-search with a full window
            if score > alpha && score < beta {
                score = -search(&new_pos, depth -1, ply + 1, -beta, -alpha, config)
            }
            score
        };

        if score >= beta {
            return beta; // Fail-hard beta cutoff
        }
        if score > alpha {
            alpha = score;
        }
    }

    alpha
}
