// src/game/search/quiescence.rs

use shakmaty::{Chess, Position};
use crate::game::evaluation;
use crate::game::evaluation::see::see;
use crate::game::search::{delta, SearchConfig, MATE_SCORE};

pub fn search(pos: &Chess, mut alpha: i32, beta: i32, config: &SearchConfig) -> i32 {
    // If delta pruning is enabled, use it for the quiescence search.
    if config.use_delta_pruning {
        return delta::search(pos, alpha, beta, config);
    }

    let standing_pat = evaluation::evaluate(pos, config);
    if standing_pat >= beta {
        return beta;
    }
    if alpha < standing_pat {
        alpha = standing_pat;
    }

    let captures = pos.legal_moves().into_iter().filter(|m| m.is_capture());

    for m in captures {
        // SEE pruning: if the capture is likely to lose material, don't search it.
        if see(pos.board(), m.from().unwrap(), m.to()) < 0 {
            continue;
        }
        let mut new_pos = pos.clone();
        new_pos.play_unchecked(m);

        // After a capture, check if the opponent has any legal moves.
        // This is necessary to spot checkmates at the end of a capture sequence.
        if new_pos.legal_moves().is_empty() {
            if new_pos.is_checkmate() {
                // We don't consider ply here, as Q-search has no ply.
                // Return a score slightly less than mate to prioritize faster mates found in the main search.
                return MATE_SCORE - 100;
            }
            return 0; // Stalemate
        }

        let score = -search(&new_pos, -beta, -alpha, config);

        if score >= beta {
            return beta;
        }
        if score > alpha {
            alpha = score;
        }
    }

    alpha
}
