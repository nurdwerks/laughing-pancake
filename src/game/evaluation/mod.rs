//! Evaluation of a chess position.

pub mod pst;
pub mod space;
pub mod pawn_structure;
pub mod advanced_pawn_structure;
pub mod mobility;
pub mod king_safety;
pub mod development;
pub mod see;
pub mod rooks;
pub mod bishops;
pub mod knights;
pub mod threats;
pub mod initiative;

use shakmaty::{Board, Chess, Color, Piece, Position, Role};
use crate::constants::{
    QUEEN_PHASE_VAL, ROOK_PHASE_VAL, BISHOP_PHASE_VAL, KNIGHT_PHASE_VAL, TOTAL_PHASE,
    PAWN_VALUE, KNIGHT_VALUE, BISHOP_VALUE, ROOK_VALUE, QUEEN_VALUE
};

/// Calculates the game phase.
///
/// The phase is a value between 0 and 256, where 256 means the game is in the
/// opening and 0 means the game is in the endgame.
fn game_phase(board: &Board) -> i32 {
    let mut current_phase_value = 0;
    for &role in &[Role::Knight, Role::Bishop, Role::Rook, Role::Queen] {
        let count = board.by_role(role).count() as i32;
        current_phase_value += count
            * match role {
                Role::Queen => QUEEN_PHASE_VAL,
                Role::Rook => ROOK_PHASE_VAL,
                Role::Bishop => BISHOP_PHASE_VAL,
                Role::Knight => KNIGHT_PHASE_VAL,
                _ => 0, // Should not happen
            };
    }
    // Clamp to TOTAL_PHASE in case of promotions
    let current_phase_value = current_phase_value.min(TOTAL_PHASE);
    (current_phase_value * 256 + (TOTAL_PHASE / 2)) / TOTAL_PHASE
}

pub fn get_piece_value(role: Role) -> i32 {
    match role {
        Role::Pawn => PAWN_VALUE,
        Role::Knight => KNIGHT_VALUE,
        Role::Bishop => BISHOP_VALUE,
        Role::Rook => ROOK_VALUE,
        Role::Queen => QUEEN_VALUE,
        Role::King => 0,
    }
}

/// Evaluates the board from the perspective of the current player.
///
/// Returns a score in centipawns.
use crate::game::search::SearchConfig;
pub fn evaluate(pos: &Chess, config: &SearchConfig) -> i32 {
    let board = pos.board();
    let phase = game_phase(board);
    let mut white_score = 0;
    let mut black_score = 0;

    // Evaluate material and PSTs for each piece
    for &color in &Color::ALL {
        for &role in &Role::ALL {
            let piece = Piece { role, color };
            let piece_bitboard = board.by_piece(piece);

            let material_value = match role {
                Role::Pawn => PAWN_VALUE,
                Role::Knight => KNIGHT_VALUE,
                Role::Bishop => BISHOP_VALUE,
                Role::Rook => ROOK_VALUE,
                Role::Queen => QUEEN_VALUE,
                Role::King => 0, // The king has no material value
            };

            let (pst_mg, pst_eg) = match (color, role) {
                (Color::White, Role::Pawn) => (pst::PAWN_PST.0, pst::PAWN_PST.1),
                (Color::White, Role::Knight) => (pst::KNIGHT_PST.0, pst::KNIGHT_PST.1),
                (Color::White, Role::Bishop) => (pst::BISHOP_PST.0, pst::BISHOP_PST.1),
                (Color::White, Role::Rook) => (pst::ROOK_PST.0, pst::ROOK_PST.1),
                (Color::White, Role::Queen) => (pst::QUEEN_PST.0, pst::QUEEN_PST.1),
                (Color::White, Role::King) => (pst::KING_PST.0, pst::KING_PST.1),
                (Color::Black, Role::Pawn) => (pst::BLACK_PAWN_PST.0, pst::BLACK_PAWN_PST.1),
                (Color::Black, Role::Knight) => (pst::BLACK_KNIGHT_PST.0, pst::BLACK_KNIGHT_PST.1),
                (Color::Black, Role::Bishop) => (pst::BLACK_BISHOP_PST.0, pst::BLACK_BISHOP_PST.1),
                (Color::Black, Role::Rook) => (pst::BLACK_ROOK_PST.0, pst::BLACK_ROOK_PST.1),
                (Color::Black, Role::Queen) => (pst::BLACK_QUEEN_PST.0, pst::BLACK_QUEEN_PST.1),
                (Color::Black, Role::King) => (pst::BLACK_KING_PST.0, pst::BLACK_KING_PST.1),
            };

            for square in piece_bitboard {
                let rank = square.rank() as usize;
                let file = square.file() as usize;

                let pst_mg_score = pst_mg[rank][file];
                let pst_eg_score = pst_eg[rank][file];

                // Interpolate between middlegame and endgame score
                let pst_score = (pst_mg_score * phase + pst_eg_score * (256 - phase)) / 256;

                let score = material_value + pst_score;

                if color == Color::White {
                    white_score += score;
                } else {
                    black_score += score;
                }
            }
        }
    }

    let total_weight = config.pawn_structure_weight
        + config.piece_mobility_weight
        + config.king_safety_weight
        + config.piece_development_weight
        + config.rook_placement_weight
        + config.bishop_placement_weight
        + config.knight_placement_weight
        + config.threat_analysis_weight
        + config.space_evaluation_weight
        + config.initiative_evaluation_weight;

    if total_weight > 0 {
        white_score += pawn_structure::evaluate(board, Color::White, config) * config.pawn_structure_weight / total_weight;
        black_score += pawn_structure::evaluate(board, Color::Black, config) * config.pawn_structure_weight / total_weight;

        white_score += mobility::evaluate(board, Color::White) * config.piece_mobility_weight / total_weight;
        black_score += mobility::evaluate(board, Color::Black) * config.piece_mobility_weight / total_weight;

        white_score += king_safety::evaluate(board, Color::White, config) * config.king_safety_weight / total_weight;
        black_score += king_safety::evaluate(board, Color::Black, config) * config.king_safety_weight / total_weight;

        white_score += development::evaluate(board, Color::White) * config.piece_development_weight / total_weight;
        black_score += development::evaluate(board, Color::Black) * config.piece_development_weight / total_weight;

        white_score += rooks::evaluate(board, Color::White) * config.rook_placement_weight / total_weight;
        black_score += rooks::evaluate(board, Color::Black) * config.rook_placement_weight / total_weight;

        white_score += bishops::evaluate(board, Color::White, config) * config.bishop_placement_weight / total_weight;
        black_score += bishops::evaluate(board, Color::Black, config) * config.bishop_placement_weight / total_weight;

        white_score += knights::evaluate(board, Color::White) * config.knight_placement_weight / total_weight;
        black_score += knights::evaluate(board, Color::Black) * config.knight_placement_weight / total_weight;

        white_score += threats::evaluate(board, Color::White) * config.threat_analysis_weight / total_weight;
        black_score += threats::evaluate(board, Color::Black) * config.threat_analysis_weight / total_weight;

        white_score += space::evaluate(board, Color::White) * config.space_evaluation_weight / total_weight;
        black_score += space::evaluate(board, Color::Black) * config.space_evaluation_weight / total_weight;

        white_score += initiative::evaluate(board, Color::White) * config.initiative_evaluation_weight / total_weight;
        black_score += initiative::evaluate(board, Color::Black) * config.initiative_evaluation_weight / total_weight;
    }

    let total_score = white_score - black_score;

    // Return score from the perspective of the current player
    let perspective_score = if pos.turn() == Color::White {
        total_score
    } else {
        -total_score
    };

    perspective_score + config.tempo_bonus_weight
}

#[cfg(test)]
pub mod tests;
