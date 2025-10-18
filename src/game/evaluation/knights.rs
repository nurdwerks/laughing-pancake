//! Evaluation terms for knights.

use shakmaty::{Board, Color, File, Piece, Role, Square, Bitboard, Rank};
use crate::constants::{OUTPOST_BONUS, CENTRALIZATION_BONUS};

/// Evaluates the placement of knights.
pub fn evaluate(board: &Board, color: Color) -> i32 {
    let mut score = 0;
    let knights = board.by_piece(Piece {
        role: Role::Knight,
        color,
    });

    for knight_square in knights {
        score += evaluate_knight(board, color, knight_square);
    }

    score
}

/// Evaluate a single knight.
fn evaluate_knight(board: &Board, color: Color, square: Square) -> i32 {
    let mut score = 0;
    score += evaluate_outpost(board, color, square);
    score += evaluate_centralization(square);
    score
}

/// Check if a knight is on an outpost.
/// An outpost is a square on the 4th, 5th, 6th, or 7th rank
/// that is protected by a friendly pawn and cannot be attacked by an enemy pawn.
fn evaluate_outpost(board: &Board, color: Color, square: Square) -> i32 {
    let rank = square.rank();
    let file = square.file();

    // Must be on a rank from 4 to 7 for white, or 4 to 1 for black
    let is_outpost_rank = match color {
        Color::White => rank >= shakmaty::Rank::Fourth && rank <= shakmaty::Rank::Seventh,
        Color::Black => rank >= shakmaty::Rank::Second && rank <= shakmaty::Rank::Fifth,
    };

    if !is_outpost_rank {
        return 0;
    }

    // Must be supported by a friendly pawn
    let friendly_pawns = board.by_piece(Piece { role: Role::Pawn, color });
    let mut is_supported = false;

    let rank_idx = square.rank() as u32;
    let file_idx = square.file() as u32;

    let back_rank_idx = if color == Color::White { rank_idx - 1 } else { rank_idx + 1 };

    if back_rank_idx < 8 {
        if file_idx > 0 && !friendly_pawns.intersect(Bitboard::from(Square::from_coords(File::new(file_idx - 1), Rank::new(back_rank_idx)))).is_empty() {
            is_supported = true;
        }
        if file_idx < 7 && !friendly_pawns.intersect(Bitboard::from(Square::from_coords(File::new(file_idx + 1), Rank::new(back_rank_idx)))).is_empty() {
            is_supported = true;
        }
    }

    if !is_supported {
        return 0;
    }

    // Cannot be attacked by an enemy pawn
    let enemy_pawns = board.by_piece(Piece { role: Role::Pawn, color: !color });

    let file_idx = file as u32;
    let mut adjacent_files = Bitboard::EMPTY;
    if file_idx > 0 {
        adjacent_files |= Bitboard::from_file(File::new(file_idx - 1));
    }
    if file_idx < 7 {
        adjacent_files |= Bitboard::from_file(File::new(file_idx + 1));
    }

    if !(enemy_pawns & adjacent_files).is_empty() {
        return 0;
    }

    OUTPOST_BONUS
}


/// Reward knights for being in the center.
fn evaluate_centralization(square: Square) -> i32 {
    let file = square.file();
    let rank = square.rank();

    // Central files are C, D, E, F
    let is_central_file = file >= File::C && file <= File::F;
    // Central ranks are 3, 4, 5, 6
    let is_central_rank = rank >= shakmaty::Rank::Third && rank <= shakmaty::Rank::Sixth;

    if is_central_file && is_central_rank {
        CENTRALIZATION_BONUS
    } else {
        0
    }
}
