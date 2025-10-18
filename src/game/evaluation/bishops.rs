//! Evaluation terms for bishops.

use shakmaty::{Board, Color, Piece, Role, Square, Bitboard};
use crate::game::search::SearchConfig;
use crate::constants::BAD_BISHOP_PENALTY;

/// Evaluates the placement of bishops.
pub fn evaluate(board: &Board, color: Color, config: &SearchConfig) -> i32 {
    let mut score = 0;
    let bishops = board.by_piece(Piece {
        role: Role::Bishop,
        color,
    });

    // Bishop pair bonus
    if bishops.count() >= 2 {
        score += config.bishop_pair_weight / 100;
    }

    // Bad bishop penalty
    for bishop_square in bishops {
        score += evaluate_bishop(board, color, bishop_square);
    }

    score
}

/// Evaluate a single bishop to check if it's a "bad" bishop.
fn evaluate_bishop(board: &Board, color: Color, square: Square) -> i32 {
    let friendly_pawns = board.by_piece(Piece {
        role: Role::Pawn,
        color,
    });

    // A bishop is considered "bad" if it is on the same color square
    // as many of the central friendly pawns.
    let central_squares = Bitboard::from(Square::C3) | Bitboard::from(Square::D3) | Bitboard::from(Square::E3) | Bitboard::from(Square::F3)
        | Bitboard::from(Square::C4) | Bitboard::from(Square::D4) | Bitboard::from(Square::E4) | Bitboard::from(Square::F4)
        | Bitboard::from(Square::C5) | Bitboard::from(Square::D5) | Bitboard::from(Square::E5) | Bitboard::from(Square::F5)
        | Bitboard::from(Square::C6) | Bitboard::from(Square::D6) | Bitboard::from(Square::E6) | Bitboard::from(Square::F6);
    let central_pawns = friendly_pawns & central_squares;

    let bishop_is_light_squared = square.is_light();
    let mut bad_bishop_pawns = 0;

    for pawn_square in central_pawns {
        if pawn_square.is_light() == bishop_is_light_squared {
            bad_bishop_pawns += 1;
        }
    }

    // The more central pawns on the same color, the worse the bishop is.
    bad_bishop_pawns * BAD_BISHOP_PENALTY
}
