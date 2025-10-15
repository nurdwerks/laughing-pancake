//! Evaluation of space control.

use shakmaty::{Board, Color, Piece, Role, Bitboard, Square, Rank, File};

const SPACE_PER_SQUARE_BONUS: i32 = 2;
const CENTER_CONTROL_BONUS: i32 = 5;

/// Evaluates the space controlled by a player.
pub fn evaluate(board: &Board, color: Color) -> i32 {
    let mut score = 0;
    let friendly_pawns = board.by_piece(Piece {
        role: Role::Pawn,
        color,
    });

    let mut attacked_squares = Bitboard::EMPTY;
    for pawn_square in friendly_pawns {
        attacked_squares |= pawn_attacks(pawn_square, color);
    }

    // Define the opponent's side of the board
    let opponent_side = match color {
        Color::White => Bitboard::from_rank(Rank::Fifth) | Bitboard::from_rank(Rank::Sixth) | Bitboard::from_rank(Rank::Seventh) | Bitboard::from_rank(Rank::Eighth),
        Color::Black => Bitboard::from_rank(Rank::First) | Bitboard::from_rank(Rank::Second) | Bitboard::from_rank(Rank::Third) | Bitboard::from_rank(Rank::Fourth),
    };

    let controlled_squares = attacked_squares & opponent_side;
    score += controlled_squares.count() as i32 * SPACE_PER_SQUARE_BONUS;

    // Bonus for controlling the center
    let center = Bitboard::from_square(Square::D4) | Bitboard::from_square(Square::E4) | Bitboard::from_square(Square::D5) | Bitboard::from_square(Square::E5);
    let center_control = attacked_squares & center;
    score += center_control.count() as i32 * CENTER_CONTROL_BONUS;

    score
}

/// Returns a bitboard of the squares a pawn attacks from a given square.
fn pawn_attacks(square: Square, color: Color) -> Bitboard {
    let mut attacks = Bitboard::EMPTY;
    let file_idx = square.file().to_u32();
    let rank_idx = square.rank().to_u32();

    match color {
        Color::White => {
            let next_rank_idx = rank_idx + 1;
            if next_rank_idx < 8 {
                let next_rank = Rank::new(next_rank_idx);
                // Left attack
                if file_idx > 0 {
                    let left_file = File::new(file_idx - 1);
                    attacks.add(Square::from_coords(left_file, next_rank));
                }
                // Right attack
                let right_file_idx = file_idx + 1;
                if right_file_idx < 8 {
                    let right_file = File::new(right_file_idx);
                    attacks.add(Square::from_coords(right_file, next_rank));
                }
            }
        }
        Color::Black => {
            if rank_idx > 0 {
                let prev_rank_idx = rank_idx - 1;
                let prev_rank = Rank::new(prev_rank_idx);
                // Left attack
                if file_idx > 0 {
                    let left_file = File::new(file_idx - 1);
                    attacks.add(Square::from_coords(left_file, prev_rank));
                }
                // Right attack
                let right_file_idx = file_idx + 1;
                if right_file_idx < 8 {
                    let right_file = File::new(right_file_idx);
                    attacks.add(Square::from_coords(right_file, prev_rank));
                }
            }
        }
    }
    attacks
}
