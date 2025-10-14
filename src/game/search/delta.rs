// src/game/search/delta.rs

//! Delta Pruning
//!
//! This is a stub for the delta pruning algorithm. Delta pruning is a forward
//! pruning technique similar to futility pruning, often used in quiescence search.
//! It is based on the idea that if the material difference between the two sides
//! is very large, then some moves (like captures of minor pieces) might not be
//! enough to change the outcome of the evaluation. If a move's potential material
//! gain plus a "delta" margin is still not enough to improve the score, it can
//! be pruned.

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
