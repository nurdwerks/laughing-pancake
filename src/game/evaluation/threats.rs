// src/game/evaluation/threats.rs

use shakmaty::{Board, Color, Piece, Role, Square};
use crate::constants::{UNDEFENDED_PIECE_BONUS_FACTOR, GOOD_TRADE_BONUS_FACTOR};
use super::get_piece_value;

pub fn evaluate(board: &Board, color: Color) -> i32 {
    let mut score = 0;
    let their_color = !color;

    // Get all squares attacked by our pieces
    let mut all_our_attacks = shakmaty::Bitboard::EMPTY;
    for sq in board.by_color(color) {
        all_our_attacks |= board.attacks_from(sq);
    }

    // Get all squares defended by their pieces
    let mut all_their_defenses = shakmaty::Bitboard::EMPTY;
    for sq in board.by_color(their_color) {
        all_their_defenses |= board.attacks_from(sq);
    }

    // Iterate through all of their pieces
    for role in Role::ALL {
        let their_pieces = board.by_piece(Piece { role, color: their_color });

        for piece_square in their_pieces {
            // Check if this piece is on a square we attack
            if !(all_our_attacks & shakmaty::Bitboard::from(piece_square)).is_empty() {
                // The piece is threatened.
                if (all_their_defenses & shakmaty::Bitboard::from(piece_square)).is_empty() {
                    // The piece is completely undefended
                    score += (get_piece_value(role) * UNDEFENDED_PIECE_BONUS_FACTOR) / 100;
                } else {
                    // The piece is defended. Check if this is a "good trade" for us.
                    if let Some(least_valuable_attacker) = find_least_valuable_attacker(board, color, piece_square) {
                        if get_piece_value(least_valuable_attacker) < get_piece_value(role) {
                             score += (get_piece_value(role) * GOOD_TRADE_BONUS_FACTOR) / 100;
                        }
                    }
                }
            }
        }
    }

    score
}

/// Finds the role of the least valuable piece of a given color that attacks a square.
fn find_least_valuable_attacker(board: &Board, color: Color, square: Square) -> Option<Role> {
    let mut least_valuable_role = None;
    let mut min_value = i32::MAX;

    for role in Role::ALL {
        if role == Role::King { continue; }
        let our_pieces = board.by_piece(Piece { role, color });
        for piece_square in our_pieces {
            let attacks = board.attacks_from(piece_square);
            if !(attacks & shakmaty::Bitboard::from(square)).is_empty() {
                let value = get_piece_value(role);
                if value < min_value {
                    min_value = value;
                    least_valuable_role = Some(role);
                }
            }
        }
    }

    least_valuable_role
}
