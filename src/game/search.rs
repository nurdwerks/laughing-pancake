// src/game/search.rs

pub mod quiescence;
pub mod pvs;
pub mod null_move;
pub mod delta;

use shakmaty::{Chess, Move, Position, uci::UciMove};
use crate::game::evaluation;

const MATE_SCORE: i32 = 1_000_000;

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SearchConfig {
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
}

impl Default for SearchConfig {
    fn default() -> Self {
        Self {
            use_quiescence_search: true,
            use_pvs: false,
            use_null_move_pruning: false,
            use_lmr: false,
            use_futility_pruning: false,
            use_delta_pruning: false,
            pawn_structure_weight: 100,
            piece_mobility_weight: 100,
            king_safety_weight: 100,
            piece_development_weight: 100,
            rook_placement_weight: 100,
            bishop_placement_weight: 100,
            knight_placement_weight: 100,
        }
    }
}

pub fn search(pos: &Chess, depth: u8, config: &SearchConfig) -> (Option<Move>, i32) {
    let mut best_move = None;
    let mut alpha = -MATE_SCORE;
    let beta = MATE_SCORE;

    let legal_moves = pos.legal_moves();

    if legal_moves.is_empty() {
        return (None, evaluation::evaluate(pos, config));
    }

    for m in legal_moves {
        let mut new_pos = pos.clone();
        new_pos.play_unchecked(m);
        let score = -alpha_beta(&new_pos, depth - 1, 1, -beta, -alpha, config);

        if score > alpha {
            alpha = score;
            best_move = Some(m);
        }
    }

    (best_move, alpha)
}

fn alpha_beta(pos: &Chess, depth: u8, ply: u8, alpha: i32, beta: i32, config: &SearchConfig) -> i32 {
    if config.use_null_move_pruning {
        null_move::search(pos, depth, ply, alpha, beta, config)
    } else {
        pvs::search(pos, depth, ply, alpha, beta, config)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use shakmaty::{fen::Fen, CastlingMode};

    #[test]
    #[ignore]
    fn test_alpha_beta_finds_mate_in_1() {
        // Position where white has a mate in 1 (Qh5#)
        let fen: Fen = "6k1/8/8/8/8/8/8/3QK2R w K - 0 1".parse().unwrap();
        let pos: Chess = fen.into_position(CastlingMode::Standard).unwrap();

        // Search depth of 3 to see the mate clearly.
        let (best_move, score) = search(&pos, 3, &SearchConfig::default());

        let mate_move_uci = UciMove::from_ascii(b"d1h5").unwrap();
        let mate_move = mate_move_uci.to_move(&pos).unwrap();

        assert_eq!(best_move, Some(mate_move));
        // The score should be MATE_SCORE - ply. Mate is at ply 1.
        assert_eq!(score, MATE_SCORE - 1);
    }

    #[test]
    #[ignore]
    fn test_pvs_finds_mate_in_1() {
        // Position where white has a mate in 1 (Qh5#)
        let fen: Fen = "6k1/8/8/8/8/8/8/3QK2R w K - 0 1".parse().unwrap();
        let pos: Chess = fen.into_position(CastlingMode::Standard).unwrap();

        let mut config = SearchConfig::default();
        config.use_pvs = true;

        // Search depth of 3 to see the mate clearly.
        let (best_move, score) = search(&pos, 3, &config);

        let mate_move_uci = UciMove::from_ascii(b"d1h5").unwrap();
        let mate_move = mate_move_uci.to_move(&pos).unwrap();

        assert_eq!(best_move, Some(mate_move));
        // The score should be MATE_SCORE - ply. Mate is at ply 1.
        assert_eq!(score, MATE_SCORE - 1);
    }

    #[test]
    fn test_alpha_beta_avoids_mate() {
        // Black to move. Kg8 allows white to mate with Ra8#.
        // Any other move (like Kg7) is better.
        let fen: Fen = "7k/8/8/8/8/8/8/R6K b - - 0 1".parse().unwrap();
        let pos: Chess = fen.into_position(CastlingMode::Standard).unwrap();

        let (best_move, _score) = search(&pos, 2, &SearchConfig::default());

        let losing_move_uci = UciMove::from_ascii(b"h8g8").unwrap();
        let losing_move = losing_move_uci.to_move(&pos).unwrap();

        assert_ne!(best_move, Some(losing_move));
    }

    #[test]
    #[ignore]
    fn test_null_move_pruning_finds_mate_in_1() {
        let fen: Fen = "6k1/8/8/8/8/8/8/3QK2R w K - 0 1".parse().unwrap();
        let pos: Chess = fen.into_position(CastlingMode::Standard).unwrap();
        let mut config = SearchConfig::default();
        config.use_null_move_pruning = true;
        let (best_move, _) = search(&pos, 3, &config);
        let mate_move = UciMove::from_ascii(b"d1h5").unwrap().to_move(&pos).unwrap();
        assert_eq!(best_move, Some(mate_move));
    }

    #[test]
    #[ignore]
    fn test_lmr_finds_mate_in_1() {
        let fen: Fen = "6k1/8/8/8/8/8/8/3QK2R w K - 0 1".parse().unwrap();
        let pos: Chess = fen.into_position(CastlingMode::Standard).unwrap();
        let mut config = SearchConfig::default();
        config.use_lmr = true;
        let (best_move, _) = search(&pos, 3, &config);
        let mate_move = UciMove::from_ascii(b"d1h5").unwrap().to_move(&pos).unwrap();
        assert_eq!(best_move, Some(mate_move));
    }

    #[test]
    #[ignore]
    fn test_futility_pruning_finds_mate_in_1() {
        let fen: Fen = "6k1/8/8/8/8/8/8/3QK2R w K - 0 1".parse().unwrap();
        let pos: Chess = fen.into_position(CastlingMode::Standard).unwrap();
        let mut config = SearchConfig::default();
        config.use_futility_pruning = true;
        let (best_move, _) = search(&pos, 3, &config);
        let mate_move = UciMove::from_ascii(b"d1h5").unwrap().to_move(&pos).unwrap();
        assert_eq!(best_move, Some(mate_move));
    }

    #[test]
    #[ignore]
    fn test_delta_pruning_in_quiescence() {
        // This test sets up a position where a capture is available,
        // but delta pruning should prune it.
        let fen: Fen = "k7/8/8/8/8/8/p1K5/R7 w - - 0 1".parse().unwrap();
        let pos: Chess = fen.into_position(CastlingMode::Standard).unwrap();
        let mut config = SearchConfig::default();
        config.use_delta_pruning = true;

        // With delta pruning, the capture of the pawn on a2 should be pruned,
        // and the best move should be something else.
        let (best_move, _) = search(&pos, 3, &config);
        let capture_move = UciMove::from_ascii(b"a1a2").unwrap().to_move(&pos).unwrap();
        assert_ne!(best_move, Some(capture_move));
    }
}
