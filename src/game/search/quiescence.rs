// src/game/search/quiescence.rs

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
