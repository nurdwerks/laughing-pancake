// src/game/search/delta.rs

//! Delta Pruning
//!
//! Delta pruning is a forward pruning technique primarily used in quiescence
//! search to reduce the search space. The core idea is to prune moves that are
//! unlikely to significantly improve the current position's score. This is
//! achieved by adding a "delta" margin (e.g., the value of a queen) to the
//! current evaluation. If this adjusted score is still below alpha, it suggests
//! that even a significant material gain from the opponent would not be enough
//! to change the outcome, so the node can be pruned. This implementation is
//! specifically used within the quiescence search.

use shakmaty::{Chess, Position};
use crate::game::evaluation;
use crate::game::evaluation::see::see;
use crate::game::search::SearchConfig;

pub fn search(pos: &Chess, mut alpha: i32, beta: i32, config: &SearchConfig) -> i32 {
    let standing_pat = evaluation::evaluate(pos, config);
    if standing_pat >= beta {
        return beta;
    }
    if alpha < standing_pat {
        alpha = standing_pat;
    }

    // --- Delta Pruning ---
    // If our score is so good that even if the opponent captures a queen,
    // we are still above beta, then we can prune this node.
    if config.use_delta_pruning {
        let queen_value = evaluation::get_piece_value(shakmaty::Role::Queen);
        if standing_pat - queen_value >= beta {
            return beta;
        }
    }

    let captures = pos.legal_moves().into_iter().filter(|m| m.is_capture());

    for m in captures {
        // SEE pruning: if the capture is likely to lose material, don't search it.
        if see(pos.board(), m.from().unwrap(), m.to()) < 0 {
            continue;
        }

        let mut new_pos = pos.clone();
        new_pos.play_unchecked(m);
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
