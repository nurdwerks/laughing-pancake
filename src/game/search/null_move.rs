// src/game/search/null_move.rs

//! Null Move Pruning
//!
//! This is a stub for the null move pruning algorithm. This is a technique
//! used to reduce the search space by assuming that if a player can make a "null"
//! move (i.e., pass their turn) and still have a score that is high enough to
//! cause a beta cutoff, then the current position is likely very strong, and
//! a full search is unnecessary. This is a powerful pruning technique but can
//! fail in zugzwang positions.

use shakmaty::{Chess, Position};

use crate::game::search::{pvs, SearchConfig};

// Constants for Null Move Pruning
const NMP_DEPTH_REDUCTION: u8 = 3;
const NMP_MIN_DEPTH: u8 = 3;

/// Performs a search with null move pruning.
/// If the conditions for NMP are met, it will try a null move and if that
/// causes a beta cutoff, the position is pruned. Otherwise, it falls back
// to a PVS search.
pub fn search(pos: &Chess, depth: u8, ply: u8, alpha: i32, beta: i32, config: &SearchConfig) -> i32 {
    // --- Null Move Pruning Heuristic ---

    // Conditions where NMP is skipped:
    // - NMP is disabled in the configuration.
    // - The search depth is too shallow.
    // - The side to move is in check (null move is illegal).
    // - The position is potentially zugzwang. A simple heuristic for this is to check if the
    //   side to move has only non-pawn material left.
    let non_pawn_material = pos.board().by_color(pos.turn()) & !pos.board().pawns();
    let is_likely_zugzwang = non_pawn_material.is_empty();

    if config.use_null_move_pruning
        && depth >= NMP_MIN_DEPTH
        && !pos.is_check()
        && !is_likely_zugzwang
    {
        // Create a new position with the turn passed.
        if let Ok(null_move_pos) = pos.clone().swap_turn() {
            // Search with reduced depth. Note the swapped beta/alpha and sign negation for the null window search.
            let score = -pvs::search(&null_move_pos, depth.saturating_sub(NMP_DEPTH_REDUCTION), ply + 1, -beta, -beta + 1, config);

        // If the null move search causes a beta cutoff, we can prune this node.
        if score >= beta {
            return beta; // Prune
        }
        }
    }

    // If NMP didn't cause a cutoff, or if the conditions were not met,
    // proceed with the standard PVS search.
    pvs::search(pos, depth, ply, alpha, beta, config)
}
