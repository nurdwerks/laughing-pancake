// src/game/evaluation/king_safety.rs

use shakmaty::{Board, Color, Piece, Role, Square, Bitboard, File, Rank};

const PAWN_SHIELD_BONUS: i32 = 30;
const OPEN_FILE_PENALTY: i32 = 25;
const SEMI_OPEN_FILE_PENALTY: i32 = 15;

pub fn evaluate(board: &Board, color: Color) -> i32 {
    let mut score = 0;
    let our_king = board.king_of(color);

    // The king might not be on the board in some test positions
    if let Some(king_square) = our_king {
        let king_file_index = king_square.file() as usize;
        let king_rank_index = king_square.rank() as usize;

        // --- Pawn Shield Evaluation ---
        let shield_rank_index = if color == Color::White {
            king_rank_index + 1
        } else {
            king_rank_index - 1
        };

        if shield_rank_index < 8 {
            let our_pawns = board.by_piece(Piece { role: Role::Pawn, color });

            // Check the pawn directly in front
            if !(our_pawns & Bitboard::from_square(Square::from_coords(king_square.file(), Rank::new(shield_rank_index as u32)))).is_empty() {
                score += PAWN_SHIELD_BONUS;
            }
            // Check adjacent files
            if king_file_index > 0 {
                if !(our_pawns & Bitboard::from_square(Square::from_coords(File::new((king_file_index - 1) as u32), Rank::new(shield_rank_index as u32)))).is_empty() {
                    score += PAWN_SHIELD_BONUS / 2;
                }
            }
            if king_file_index < 7 {
                if !(our_pawns & Bitboard::from_square(Square::from_coords(File::new((king_file_index + 1) as u32), Rank::new(shield_rank_index as u32)))).is_empty() {
                    score += PAWN_SHIELD_BONUS / 2;
                }
            }
        }

        // --- Open File Evaluation ---
        let our_pawns = board.by_piece(Piece { role: Role::Pawn, color });
        let their_pawns = board.by_piece(Piece { role: Role::Pawn, color: !color });

        for file_index in (king_file_index.saturating_sub(1))..=(king_file_index + 1).min(7) {
            let file_bb = Bitboard::from_file(File::new(file_index as u32));
            let our_pawns_on_file = (our_pawns & file_bb).is_empty();
            let their_pawns_on_file = (their_pawns & file_bb).is_empty();

            if our_pawns_on_file && their_pawns_on_file {
                // Open file
                score -= OPEN_FILE_PENALTY;
            } else if our_pawns_on_file && !their_pawns_on_file {
                // Semi-open file for the opponent
                score -= SEMI_OPEN_FILE_PENALTY;
            }
        }
    }

    score
}
