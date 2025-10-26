use shakmaty::{attacks, Bitboard, Board, Color, Piece, Role, Square};
use crate::constants::{GOOD_TRADE_BONUS_FACTOR, TACTICAL_PRESSURE_BONUS, UNDEFENDED_PIECE_BONUS_FACTOR};
use super::get_piece_value;

pub fn evaluate(board: &Board, color: Color) -> i32 {
    let mut score = 0;
    let their_color = !color;
    let occupied = board.occupied();

    // Get all squares attacked by our pieces
    let mut all_our_attacks = Bitboard::EMPTY;
    for sq in board.by_color(color) {
        if let Some(piece) = board.piece_at(sq) {
            all_our_attacks |= get_attacks(piece.role, sq, color, occupied);
        }
    }

    // Get all squares defended by their pieces
    let mut all_their_defenses = Bitboard::EMPTY;
    for sq in board.by_color(their_color) {
        if let Some(piece) = board.piece_at(sq) {
            all_their_defenses |= get_attacks(piece.role, sq, their_color, occupied);
        }
    }

    // Iterate through all of their pieces
    for role in Role::ALL {
        let their_pieces = board.by_piece(Piece { role, color: their_color });

        for piece_square in their_pieces {
            // Check if this piece is on a square we attack
            if !(all_our_attacks & Bitboard::from(piece_square)).is_empty() {
                // The piece is threatened.
                if (all_their_defenses & Bitboard::from(piece_square)).is_empty() {
                    // The piece is completely undefended
                    score += (get_piece_value(role) * UNDEFENDED_PIECE_BONUS_FACTOR) / 100;
                } else {
                    // The piece is defended. Check for good trades.
                    if let Some(least_valuable_attacker) = find_least_valuable_attacker(board, color, piece_square) {
                        if get_piece_value(least_valuable_attacker) < get_piece_value(role) {
                             score += (get_piece_value(role) * GOOD_TRADE_BONUS_FACTOR) / 100;
                        }
                    }
                    // Add a small bonus for creating any threat, even on a defended piece.
                    score += TACTICAL_PRESSURE_BONUS;
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
    let occupied = board.occupied();

    for role in Role::ALL {
        if role == Role::King { continue; }
        let our_pieces = board.by_piece(Piece { role, color });
        for piece_square in our_pieces {
            let attacks = get_attacks(role, piece_square, color, occupied);
            if !(attacks & Bitboard::from(square)).is_empty() {
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

fn get_attacks(role: Role, sq: Square, color: Color, occupied: Bitboard) -> Bitboard {
    match role {
        Role::Pawn => attacks::pawn_attacks(color, sq),
        Role::Knight => attacks::knight_attacks(sq),
        Role::Bishop => attacks::bishop_attacks(sq, occupied),
        Role::Rook => attacks::rook_attacks(sq, occupied),
        Role::Queen => attacks::queen_attacks(sq, occupied),
        Role::King => attacks::king_attacks(sq),
    }
}
