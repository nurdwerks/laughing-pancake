// src/game/evaluation/pawn_structure.rs

use shakmaty::{Board, Color, Piece, Role, Bitboard, File};
use crate::game::search::SearchConfig;
use super::advanced_pawn_structure;

pub fn evaluate(board: &Board, color: Color, config: &SearchConfig) -> i32 {
    let mut score = 0;
    let our_pawns = board.by_piece(Piece { role: Role::Pawn, color });

    score -= count_doubled_pawns(our_pawns) * config.doubled_pawn_weight / 100;
    score -= count_isolated_pawns(our_pawns) * config.isolated_pawn_weight / 100;

    // Add scores from advanced pawn structure analysis
    score += advanced_pawn_structure::evaluate_pawn_chains(board, color) * config.pawn_chain_weight / 100;
    score += advanced_pawn_structure::evaluate_rams(board, color) * config.ram_weight / 100;
    score += advanced_pawn_structure::evaluate_candidate_passed_pawns(board, color) * config.candidate_passed_pawn_weight / 100;

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
        if file_index > 0 && !(our_pawns & Bitboard::from_file(File::new((file_index - 1) as u32))).is_empty() {
            has_friendly_pawn_on_adjacent_file = true;
        }
        if file_index < 7 && !(our_pawns & Bitboard::from_file(File::new((file_index + 1) as u32))).is_empty() {
            has_friendly_pawn_on_adjacent_file = true;
        }
        if !has_friendly_pawn_on_adjacent_file {
            isolated_pawns += 1;
        }
    }
    isolated_pawns
}
