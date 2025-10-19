use shakmaty::{attacks, Bitboard, Board, Color, File, Piece, Rank, Role, Square};

const PASSED_PAWN_RANK_BONUS: [i32; 8] = [0, 5, 10, 20, 35, 60, 100, 0];
const SUPPORTED_PASSED_PAWN_BONUS: i32 = 15;
const PATH_CLARITY_BONUS: i32 = 5;

pub fn evaluate(board: &Board, color: Color) -> i32 {
    let mut score = 0;
    let our_pawns = board.by_piece(Piece { role: Role::Pawn, color });
    let their_pawns = board.by_piece(Piece { role: Role::Pawn, color: !color });

    for pawn_square in our_pawns {
        if is_passed_pawn(pawn_square, color, their_pawns) {
            let rank = pawn_square.rank() as u32;
            let bonus = if color == Color::White {
                PASSED_PAWN_RANK_BONUS[rank as usize]
            } else {
                PASSED_PAWN_RANK_BONUS[7 - rank as usize]
            };
            score += bonus;

            // Check for support from other pawns
            if is_supported_by_pawn(pawn_square, color, our_pawns) {
                score += SUPPORTED_PASSED_PAWN_BONUS;
            }

            // Check path clarity
            score += evaluate_path_clarity(board, pawn_square, color);
        }
    }
    score
}

fn is_passed_pawn(pawn_square: Square, color: Color, their_pawns: Bitboard) -> bool {
    let file_idx = pawn_square.file() as u32;
    let rank_idx = pawn_square.rank() as u32;

    let mut in_front_files = Bitboard::from_file(pawn_square.file());
    if file_idx > 0 {
        in_front_files |= Bitboard::from_file(File::new(file_idx - 1));
    }
    if file_idx < 7 {
        in_front_files |= Bitboard::from_file(File::new(file_idx + 1));
    }

    let mut in_front_squares = Bitboard::EMPTY;
    match color {
        Color::White => {
            for r in (rank_idx + 1)..8 {
                in_front_squares |= Bitboard::from_rank(Rank::new(r));
            }
        }
        Color::Black => {
            for r in 0..rank_idx {
                in_front_squares |= Bitboard::from_rank(Rank::new(r));
            }
        }
    }

    (their_pawns & in_front_files & in_front_squares).is_empty()
}

fn is_supported_by_pawn(pawn_square: Square, color: Color, our_pawns: Bitboard) -> bool {
    let rank_idx = pawn_square.rank() as u32;
    let support_rank_idx = if color == Color::White {
        rank_idx.checked_sub(1)
    } else {
        rank_idx.checked_add(1)
    };

    if support_rank_idx.is_none() || support_rank_idx.unwrap() > 7 {
        return false;
    }
    let support_rank = Rank::new(support_rank_idx.unwrap());
    let file_idx = pawn_square.file() as u32;

    let mut support_squares = Bitboard::EMPTY;
    if file_idx > 0 {
        support_squares.set(Square::from_coords(File::new(file_idx - 1), support_rank), true);
    }
    if file_idx < 7 {
        support_squares.set(Square::from_coords(File::new(file_idx + 1), support_rank), true);
    }

    !(our_pawns & support_squares).is_empty()
}

fn evaluate_path_clarity(board: &Board, pawn_square: Square, color: Color) -> i32 {
    let mut clarity_score = 0;
    let their_color = !color;
    let promotion_rank = if color == Color::White { Rank::Eighth } else { Rank::First };
    let promotion_square = Square::from_coords(pawn_square.file(), promotion_rank);

    let mut path_squares = Bitboard::EMPTY;
    let rank_idx = pawn_square.rank() as u32;
    if color == Color::White {
        for r in (rank_idx + 1)..8 {
            path_squares.set(Square::from_coords(pawn_square.file(), Rank::new(r)), true);
        }
    } else {
        for r in 0..rank_idx {
            path_squares.set(Square::from_coords(pawn_square.file(), Rank::new(r)), true);
        }
    }

    for path_sq in path_squares {
        if !is_square_attacked_by(board, path_sq, their_color) {
            clarity_score += PATH_CLARITY_BONUS;
        }
    }

    // Bonus if promotion square is not controlled by the enemy king
    if let Some(their_king_sq) = board.king_of(their_color) {
        if attacks::king_attacks(their_king_sq).intersect(Bitboard::from_square(promotion_square)).is_empty() {
            clarity_score += PATH_CLARITY_BONUS * 2;
        }
    }

    clarity_score
}

fn is_square_attacked_by(board: &Board, square: Square, color: Color) -> bool {
    let occupied = board.occupied();
    let attackers = board.by_color(color);

    if !(attacks::pawn_attacks(!color, square) & attackers & board.pawns()).is_empty() {
        return true;
    }
    if !(attacks::knight_attacks(square) & attackers & board.knights()).is_empty() {
        return true;
    }
    if !(attacks::bishop_attacks(square, occupied) & attackers & board.bishops()).is_empty() {
        return true;
    }
    if !(attacks::rook_attacks(square, occupied) & attackers & board.rooks()).is_empty() {
        return true;
    }
    if !(attacks::queen_attacks(square, occupied) & attackers & board.queens()).is_empty() {
        return true;
    }
    if !(attacks::king_attacks(square) & attackers & board.kings()).is_empty() {
        return true;
    }

    false
}
