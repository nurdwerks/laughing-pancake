// src/game/search/quiescence.rs

use shakmaty::{Chess, Position};
use crate::game::evaluation;

pub fn search(pos: &Chess, mut alpha: i32, beta: i32) -> i32 {
    let standing_pat = evaluation::evaluate(pos);
    if standing_pat >= beta {
        return beta;
    }
    if alpha < standing_pat {
        alpha = standing_pat;
    }

    let captures = pos.legal_moves().into_iter().filter(|m| m.is_capture());

    for m in captures {
        let mut new_pos = pos.clone();
        new_pos.play_unchecked(m);
        let score = -search(&new_pos, -beta, -alpha);

        if score >= beta {
            return beta;
        }
        if score > alpha {
            alpha = score;
        }
    }

    alpha
}
