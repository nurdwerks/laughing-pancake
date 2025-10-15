// src/game/evaluation/advanced_pawn_structure.rs

use shakmaty::{Board, Color, Piece, Role, Bitboard, File, Rank, Square};

const PAWN_CHAIN_BONUS: i32 = 10;
const RAM_PENALTY: i32 = -5;
const CANDIDATE_PASSED_PAWN_BONUS: i32 = 15;

/// Evaluates pawn chains for a given color.
/// A pawn chain is a pawn defended by another pawn.
pub fn evaluate_pawn_chains(board: &Board, color: Color) -> i32 {
    let mut score = 0;
    let our_pawns = board.by_piece(Piece { role: Role::Pawn, color });

    for pawn_square in our_pawns {
        let rank_idx = pawn_square.rank().to_u32();
        if let Some(defending_rank_idx) = if color == Color::White { rank_idx.checked_sub(1) } else { rank_idx.checked_add(1) } {
            if defending_rank_idx < 8 {
                let defending_rank = Rank::new(defending_rank_idx);
                let file_idx = pawn_square.file().to_u32();

                // Check for defenders on adjacent files in the rank behind
                if let Some(left_file_idx) = file_idx.checked_sub(1) {
                    let defender_square = Square::from_coords(File::new(left_file_idx), defending_rank);
                    if !(our_pawns & Bitboard::from(defender_square)).is_empty() {
                        score += PAWN_CHAIN_BONUS;
                    }
                }
                if let Some(right_file_idx) = file_idx.checked_add(1) {
                    if right_file_idx < 8 {
                        let defender_square = Square::from_coords(File::new(right_file_idx), defending_rank);
                        if !(our_pawns & Bitboard::from(defender_square)).is_empty() {
                            score += PAWN_CHAIN_BONUS;
                        }
                    }
                }
            }
        }
    }
    score
}

/// Evaluates pawn rams for a given color.
/// A ram is a pawn that is blocked by an opposing pawn.
pub fn evaluate_rams(board: &Board, color: Color) -> i32 {
    let mut score = 0;
    let our_pawns = board.by_piece(Piece { role: Role::Pawn, color });
    let their_pawns = board.by_piece(Piece { role: Role::Pawn, color: !color });

    for pawn_square in our_pawns {
        let rank_idx = pawn_square.rank().to_u32();
        if let Some(blocking_rank_idx) = if color == Color::White { rank_idx.checked_add(1) } else { rank_idx.checked_sub(1) } {
            if blocking_rank_idx < 8 {
                let blocking_rank = Rank::new(blocking_rank_idx);
                let blocking_square = Square::from_coords(pawn_square.file(), blocking_rank);
                if !(their_pawns & Bitboard::from(blocking_square)).is_empty() {
                    score += RAM_PENALTY;
                }
            }
        }
    }
    score
}


/// Evaluates candidate passed pawns.
/// A candidate passed pawn has no enemy pawns in front of it on its file or adjacent files.
pub fn evaluate_candidate_passed_pawns(board: &Board, color: Color) -> i32 {
    let mut score = 0;
    let our_pawns = board.by_piece(Piece { role: Role::Pawn, color });
    let their_pawns = board.by_piece(Piece { role: Role::Pawn, color: !color });

    for pawn_square in our_pawns {
        let file_index = pawn_square.file() as usize;
        let rank_index = pawn_square.rank() as usize;

        // Create a bitboard representing the files in front (same file and adjacent files)
        let mut front_files = Bitboard::from_file(pawn_square.file());
        if file_index > 0 {
            front_files |= Bitboard::from_file(File::new((file_index - 1) as u32));
        }
        if file_index < 7 {
            front_files |= Bitboard::from_file(File::new((file_index + 1) as u32));
        }

        // Create a bitboard representing the squares in front of the pawn
        let mut front_squares = Bitboard::EMPTY;
        match color {
            Color::White => {
                for r in (rank_index + 1)..8 {
                    front_squares |= Bitboard::from_rank(Rank::new(r as u32));
                }
            }
            Color::Black => {
                for r in 0..rank_index {
                    front_squares |= Bitboard::from_rank(Rank::new(r as u32));
                }
            }
        }

        // Check if there are any enemy pawns in the path
        let enemy_pawns_in_path = their_pawns & front_files & front_squares;
        if enemy_pawns_in_path.is_empty() {
            score += CANDIDATE_PASSED_PAWN_BONUS;
        }
    }
    score
}
