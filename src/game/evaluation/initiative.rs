//! Evaluation of initiative and threats.

use shakmaty::{Board, Color, Piece, Role, Bitboard};

const THREAT_ON_MINOR_PIECE_BONUS: i32 = 10;
const THREAT_ON_MAJOR_PIECE_BONUS: i32 = 25;

/// Evaluates the initiative for a player.
///
/// This implementation defines initiative as making threats against the
/// opponent's pieces. A bonus is awarded for each piece that is attacked.
pub fn evaluate(board: &Board, color: Color) -> i32 {
    let mut score = 0;
    let opponent_color = !color;

    // Get all squares attacked by the current player
    let mut attacked_squares = Bitboard::EMPTY;
    for role in Role::ALL {
        for square in board.by_piece(Piece { role, color }) {
            attacked_squares |= board.attacks_from(square);
        }
    }

    // Check for attacks on minor pieces
    let minor_pieces = board.by_role(Role::Knight) | board.by_role(Role::Bishop);
    let opponent_minors = minor_pieces & board.by_color(opponent_color);
    let threatened_minors = attacked_squares & opponent_minors;
    score += threatened_minors.count() as i32 * THREAT_ON_MINOR_PIECE_BONUS;

    // Check for attacks on major pieces
    let major_pieces = board.by_role(Role::Rook) | board.by_role(Role::Queen);
    let opponent_majors = major_pieces & board.by_color(opponent_color);
    let threatened_majors = attacked_squares & opponent_majors;
    score += threatened_majors.count() as i32 * THREAT_ON_MAJOR_PIECE_BONUS;

    score
}
