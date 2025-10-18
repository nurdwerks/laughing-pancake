// src/game/evaluation/mobility.rs

use shakmaty::{Board, Color, Piece, Role};
use crate::constants::{
    KNIGHT_MOBILITY_BONUS, BISHOP_MOBILITY_BONUS, ROOK_MOBILITY_BONUS, QUEEN_MOBILITY_BONUS
};

pub fn evaluate(board: &Board, color: Color) -> i32 {
    let mut mobility_score = 0;
    let occupied = board.occupied();
    let friendly_pieces = board.by_color(color);

    for role in [Role::Knight, Role::Bishop, Role::Rook, Role::Queen] {
        let piece = Piece { role, color };
        let piece_bitboard = board.by_piece(piece);

        let mobility_bonus = match role {
            Role::Knight => KNIGHT_MOBILITY_BONUS,
            Role::Bishop => BISHOP_MOBILITY_BONUS,
            Role::Rook => ROOK_MOBILITY_BONUS,
            Role::Queen => QUEEN_MOBILITY_BONUS,
            _ => 0,
        };

        for square in piece_bitboard {
            let attacks = shakmaty::attacks::attacks(square, piece, occupied);
            let valid_moves = attacks & !friendly_pieces;
            mobility_score += valid_moves.count() as i32 * mobility_bonus;
        }
    }

    mobility_score
}
