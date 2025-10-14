//! Evaluation terms for rooks.

use shakmaty::{Board, Color, File, Piece, Role, Square, Bitboard};

const OPEN_FILE_BONUS: i32 = 20;
const SEMI_OPEN_FILE_BONUS: i32 = 10;
const SEVENTH_RANK_BONUS: i32 = 25;

/// Evaluates the placement of rooks.
pub fn evaluate(board: &Board, color: Color) -> i32 {
    let mut score = 0;
    let rooks = board.by_piece(Piece {
        role: Role::Rook,
        color,
    });

    for rook_square in rooks {
        score += evaluate_rook(board, color, rook_square);
    }

    score
}

/// Evaluate a single rook.
fn evaluate_rook(board: &Board, color: Color, square: Square) -> i32 {
    let mut score = 0;
    let file = square.file();

    score += evaluate_file(board, color, file);
    score += evaluate_rank(board, color, square);

    score
}

/// Evaluate the file the rook is on.
fn evaluate_file(board: &Board, color: Color, file: File) -> i32 {
    let friendly_pawns = board.by_piece(Piece {
        role: Role::Pawn,
        color,
    });
    let enemy_pawns = board.by_piece(Piece {
        role: Role::Pawn,
        color: !color,
    });

    let file_bb = Bitboard::from_file(file);

    let has_friendly_pawns = !(friendly_pawns & file_bb).is_empty();
    let has_enemy_pawns = !(enemy_pawns & file_bb).is_empty();

    if !has_friendly_pawns {
        if !has_enemy_pawns {
            OPEN_FILE_BONUS
        } else {
            SEMI_OPEN_FILE_BONUS
        }
    } else {
        0
    }
}

/// Evaluate the rank the rook is on.
fn evaluate_rank(_board: &Board, color: Color, square: Square) -> i32 {
    let rank = square.rank();
    let seventh_rank = if color == Color::White {
        shakmaty::Rank::Seventh
    } else {
        shakmaty::Rank::Second
    };

    if rank == seventh_rank {
        SEVENTH_RANK_BONUS
    } else {
        0
    }
}
