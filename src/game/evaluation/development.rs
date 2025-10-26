// src/game/evaluation/development.rs

use shakmaty::{Board, Color, Piece, Role, Square, Bitboard};
use crate::constants::{DEVELOPMENT_BONUS_MINOR, EARLY_QUEEN_MOVE_PENALTY};

pub fn evaluate(board: &Board, color: Color) -> i32 {
    let mut score = 0;

    let (knight_starts, bishop_starts, queen_start) = if color == Color::White {
        (
            Bitboard::from_square(Square::B1) | Bitboard::from_square(Square::G1),
            Bitboard::from_square(Square::C1) | Bitboard::from_square(Square::F1),
            Square::D1,
        )
    } else {
        (
            Bitboard::from_square(Square::B8) | Bitboard::from_square(Square::G8),
            Bitboard::from_square(Square::C8) | Bitboard::from_square(Square::F8),
            Square::D8,
        )
    };

    // Bonus for developed knights
    let knights = board.by_piece(Piece { role: Role::Knight, color });
    let developed_knights = (knights & !knight_starts).count();
    score += developed_knights as i32 * DEVELOPMENT_BONUS_MINOR;

    // Bonus for developed bishops
    let bishops = board.by_piece(Piece { role: Role::Bishop, color });
    let developed_bishops = (bishops & !bishop_starts).count();
    score += developed_bishops as i32 * DEVELOPMENT_BONUS_MINOR;

    // Penalty for early queen move
    let queen = board.by_piece(Piece { role: Role::Queen, color });
    if !queen.is_empty() && (queen & Bitboard::from_square(queen_start)).is_empty() {
        score -= EARLY_QUEEN_MOVE_PENALTY;
    }

    score
}
