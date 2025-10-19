use shakmaty::{attacks, Bitboard, Chess, Color, Position, Role, Square};

const ATTACKER_VALUES: [i32; 7] = [
    0, // Unused
    1, // Pawn
    3, // Knight
    3, // Bishop
    5, // Rook
    9, // Queen
    0, // King
];

/// Calculates the king attack score for the given color.
/// The score is based on the number and type of pieces attacking the opponent's king zone.
pub fn evaluate(pos: &Chess, attacker_color: Color) -> i32 {
    let board = pos.board();
    let opponent_king_sq = match board.king_of(!attacker_color) {
        Some(sq) => sq,
        None => return 0, // Should not happen in a legal position
    };

    let king_zone = get_king_zone(opponent_king_sq);
    let mut total_attack_score = 0;
    let occupied = board.occupied();

    for sq in board.by_color(attacker_color) {
        if let Some(piece) = board.piece_at(sq) {
            let attacks = match piece.role {
                Role::Pawn => attacks::pawn_attacks(attacker_color, sq),
                Role::Knight => attacks::knight_attacks(sq),
                Role::Bishop => attacks::bishop_attacks(sq, occupied),
                Role::Rook => attacks::rook_attacks(sq, occupied),
                Role::Queen => attacks::queen_attacks(sq, occupied),
                Role::King => attacks::king_attacks(sq),
            };
            let attacked_zone_squares = attacks & king_zone;

            if !attacked_zone_squares.is_empty() {
                total_attack_score += ATTACKER_VALUES[piece.role as usize];
            }
        }
    }
    total_attack_score
}

/// Defines a 2-square radius zone around the king.
fn get_king_zone(king_sq: Square) -> Bitboard {
    let mut king_zone = Bitboard::EMPTY;
    let king_moves = attacks::king_attacks(king_sq);
    king_zone |= king_moves;
    king_zone.set(king_sq, true);

    // Add a second ring of squares
    for sq in king_moves {
        king_zone |= attacks::king_attacks(sq);
    }

    king_zone
}
