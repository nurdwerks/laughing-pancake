// src/game/evaluation/king_safety.rs

use shakmaty::{Board, Color, Piece, Role, Square, Bitboard, File, Rank};
use crate::game::search::SearchConfig;
use super::get_piece_value;

pub fn evaluate(board: &Board, color: Color, config: &SearchConfig) -> i32 {
    let mut score = 0;
    let our_king = board.king_of(color);

    if let Some(king_square) = our_king {
        // --- Pawn Shield Evaluation ---
        score += evaluate_pawn_shield(board, color, king_square) * config.king_pawn_shield_weight / 100;

        // --- Open File Evaluation ---
        score -= evaluate_open_files(board, color, king_square) * config.king_open_file_penalty / 100;

        // --- Attacker Evaluation ---
        score -= evaluate_attackers(board, color, king_square) * config.king_attackers_weight / 100;
    }

    score
}

fn evaluate_pawn_shield(board: &Board, color: Color, king_square: Square) -> i32 {
    let mut shield_score = 0;
    let king_file_index = king_square.file() as usize;
    let king_rank_index = king_square.rank() as usize;

    let shield_rank_index = if color == Color::White {
        king_rank_index + 1
    } else {
        king_rank_index.saturating_sub(1)
    };

    if shield_rank_index < 8 {
        let our_pawns = board.by_piece(Piece { role: Role::Pawn, color });
        const PAWN_SHIELD_BONUS: i32 = 30; // Base value

        let shield_rank = Rank::new(shield_rank_index as u32);

        // Check the pawn directly in front
        if !(our_pawns & Bitboard::from(Square::from_coords(king_square.file(), shield_rank))).is_empty() {
            shield_score += PAWN_SHIELD_BONUS;
        }
        // Check adjacent files
        if king_file_index > 0 {
             if !(our_pawns & Bitboard::from(Square::from_coords(File::new((king_file_index - 1) as u32), shield_rank))).is_empty() {
                shield_score += PAWN_SHIELD_BONUS / 2;
            }
        }
        if king_file_index < 7 {
            if !(our_pawns & Bitboard::from(Square::from_coords(File::new((king_file_index + 1) as u32), shield_rank))).is_empty() {
                shield_score += PAWN_SHIELD_BONUS / 2;
            }
        }
    }
    shield_score
}


fn evaluate_open_files(board: &Board, color: Color, king_square: Square) -> i32 {
    let mut open_file_penalty = 0;
    let king_file_index = king_square.file() as usize;

    let our_pawns = board.by_piece(Piece { role: Role::Pawn, color });
    let their_pawns = board.by_piece(Piece { role: Role::Pawn, color: !color });

    const OPEN_FILE_PENALTY: i32 = 25;
    const SEMI_OPEN_FILE_PENALTY: i32 = 15;

    for file_index in (king_file_index.saturating_sub(1))..=(king_file_index + 1).min(7) {
        let file_bb = Bitboard::from_file(File::new(file_index as u32));
        let no_our_pawns_on_file = (our_pawns & file_bb).is_empty();
        let no_their_pawns_on_file = (their_pawns & file_bb).is_empty();

        if no_our_pawns_on_file && no_their_pawns_on_file {
            open_file_penalty += OPEN_FILE_PENALTY;
        } else if no_our_pawns_on_file && !no_their_pawns_on_file {
            open_file_penalty += SEMI_OPEN_FILE_PENALTY;
        }
    }
    open_file_penalty
}

fn evaluate_attackers(board: &Board, color: Color, king_square: Square) -> i32 {
    let mut attacker_score = 0;
    let their_color = !color;

    // Define the king zone (a 3x3 square around the king)
    let mut king_zone = Bitboard::EMPTY;
    let king_file_idx = king_square.file().to_u32();
    let king_rank_idx = king_square.rank().to_u32();

    for file_offset in -1..=1 {
        for rank_offset in -1..=1 {
            let file = king_file_idx as i32 + file_offset;
            let rank = king_rank_idx as i32 + rank_offset;
            if (0..=7).contains(&file) && (0..=7).contains(&rank) {
                 king_zone.add(Square::from_coords(File::new(file as u32), Rank::new(rank as u32)));
            }
        }
    }

    // Check for each enemy piece if it attacks the king zone
    for piece_square in board.by_color(their_color) {
        if let Some(piece) = board.piece_at(piece_square) {
            // Pawns are handled by their own evaluation terms mostly
            if piece.role == Role::Pawn || piece.role == Role::King {
                continue;
            }
            let attacks = board.attacks_from(piece_square);
            if !(attacks & king_zone).is_empty() {
                // Attacker found, add its value to the score
                // We use a smaller value than the material value for the penalty
                attacker_score += get_piece_value(piece.role) / 4;
            }
        }
    }

    attacker_score
}
