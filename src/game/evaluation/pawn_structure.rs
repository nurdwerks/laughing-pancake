// src/game/evaluation/pawn_structure.rs

use shakmaty::{Board, Color, Piece, Role, Bitboard, File, Rank};

const DOUBLED_PAWN_PENALTY: i32 = 10;
const ISOLATED_PAWN_PENALTY: i32 = 20;
const PASSED_PAWN_BONUS: i32 = 50;

pub fn evaluate(board: &Board, color: Color) -> i32 {
    let mut score = 0;
    let our_pawns = board.by_piece(Piece { role: Role::Pawn, color });
    let their_pawns = board.by_piece(Piece { role: Role::Pawn, color: !color });

    score -= count_doubled_pawns(our_pawns) * DOUBLED_PAWN_PENALTY;
    score -= count_isolated_pawns(our_pawns) * ISOLATED_PAWN_PENALTY;
    score += count_passed_pawns(color, our_pawns, their_pawns) * PASSED_PAWN_BONUS;

    score
}

fn count_doubled_pawns(our_pawns: Bitboard) -> i32 {
    let mut doubled_pawns = 0;
    for file in File::ALL {
        let pawns_on_file = (our_pawns & Bitboard::from_file(file)).count();
        if pawns_on_file > 1 {
            doubled_pawns += pawns_on_file - 1;
        }
    }
    doubled_pawns as i32
}

fn count_isolated_pawns(our_pawns: Bitboard) -> i32 {
    let mut isolated_pawns = 0;
    for pawn_square in our_pawns {
        let file_index = pawn_square.file() as usize;
        let mut has_friendly_pawn_on_adjacent_file = false;
        if file_index > 0 {
            if !(our_pawns & Bitboard::from_file(File::new((file_index - 1) as u32))).is_empty() {
                has_friendly_pawn_on_adjacent_file = true;
            }
        }
        if file_index < 7 {
            if !(our_pawns & Bitboard::from_file(File::new((file_index + 1) as u32))).is_empty() {
                has_friendly_pawn_on_adjacent_file = true;
            }
        }
        if !has_friendly_pawn_on_adjacent_file {
            isolated_pawns += 1;
        }
    }
    isolated_pawns as i32
}

fn count_passed_pawns(color: Color, our_pawns: Bitboard, their_pawns: Bitboard) -> i32 {
    let mut passed_pawns = 0;
    for pawn_square in our_pawns {
        let file_index = pawn_square.file() as usize;
        let rank_index = pawn_square.rank() as usize;

        let mut in_front_files = Bitboard::from_file(pawn_square.file());
        if file_index > 0 {
            in_front_files |= Bitboard::from_file(File::new((file_index - 1) as u32));
        }
        if file_index < 7 {
            in_front_files |= Bitboard::from_file(File::new((file_index + 1) as u32));
        }

        let mut in_front_squares = Bitboard::EMPTY;
        match color {
            Color::White => {
                for r in (rank_index + 1)..8 {
                    in_front_squares |= Bitboard::from_rank(Rank::new(r as u32));
                }
            }
            Color::Black => {
                for r in 0..rank_index {
                    in_front_squares |= Bitboard::from_rank(Rank::new(r as u32));
                }
            }
        }

        let enemy_pawns_in_front = their_pawns & in_front_files & in_front_squares;
        if enemy_pawns_in_front.is_empty() {
            passed_pawns += 1;
        }
    }
    passed_pawns as i32
}
