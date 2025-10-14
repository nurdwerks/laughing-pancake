// src/game/evaluation/see.rs

use shakmaty::{Board, Color, Role, Square};

fn get_piece_value(role: Role) -> i32 {
    match role {
        Role::Pawn => 100,
        Role::Knight => 320,
        Role::Bishop => 330,
        Role::Rook => 500,
        Role::Queen => 900,
        Role::King => 10_000,
    }
}

/// Static Exchange Evaluation (SEE)
///
/// Determines the likely outcome of a series of exchanges on a given square for a given move.
/// The `attacker_square` is the square of the piece moving to `target_square`.
/// Returns a score indicating the material gain or loss. A positive score means the exchange is favorable.
pub fn see(board: &Board, attacker_square: Square, target_square: Square) -> i32 {
    let attacker_piece = if let Some(p) = board.piece_at(attacker_square) {
        p
    } else {
        return 0; // No attacker
    };

    let captured_piece = board.piece_at(target_square);

    let mut gain = 0;
    if let Some(captured) = captured_piece {
        gain = get_piece_value(captured.role);
    }

    // Make a hypothetical move on a cloned board
    let mut next_board = board.clone();
    next_board.discard_piece_at(attacker_square);
    next_board.set_piece_at(target_square, attacker_piece);

    // The initial capture is worth the value of the captured piece,
    // minus the value of the subsequent exchange from the opponent's perspective.
    gain - see_recursive(&next_board, target_square, !attacker_piece.color)
}

fn see_recursive(board: &Board, target_square: Square, color: Color) -> i32 {
    let attackers = board.attacks_to(target_square, color, board.occupied()) & board.by_color(color);
    if attackers.is_empty() {
        return 0; // No more attackers for this side, exchange ends.
    }

    // Find the least valuable attacker
    let mut least_valuable_attacker_square = None;
    let mut min_value = i32::MAX;

    for sq in attackers {
        if let Some(piece) = board.piece_at(sq) {
            let value = get_piece_value(piece.role);
            if value < min_value {
                min_value = value;
                least_valuable_attacker_square = Some(sq);
            }
        }
    }

    let attacker_square = least_valuable_attacker_square.unwrap();
    let attacker_piece = board.piece_at(attacker_square).unwrap();
    let captured_piece = board.piece_at(target_square).unwrap(); // The piece that is on the square now

    // Make a hypothetical move on a cloned board
    let mut next_board = board.clone();
    next_board.discard_piece_at(attacker_square);
    next_board.set_piece_at(target_square, attacker_piece);

    // The value of this part of the exchange is the piece we capture,
    // minus the value of the opponent's response.
    get_piece_value(captured_piece.role) - see_recursive(&next_board, target_square, !color)
}
