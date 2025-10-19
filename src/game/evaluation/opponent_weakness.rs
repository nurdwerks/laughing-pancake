use shakmaty::{attacks, Bitboard, Board, Chess, Color, File, Piece, Position, Rank, Role, Square};

const BACKWARD_PAWN_PENALTY: i32 = 5;
const ATTACK_ON_WEAK_PAWN_BONUS: i32 = 10;
const WEAK_SQUARE_CONTROL_BONUS: i32 = 8;
const PINNED_PIECE_BONUS: i32 = 20;

pub fn evaluate(pos: &Chess, color: Color) -> i32 {
    let mut score = 0;
    let board = pos.board();
    let their_color = !color;

    // 1. Evaluate backward pawns for the opponent
    score += evaluate_backward_pawns(board, their_color) * BACKWARD_PAWN_PENALTY;

    // 2. Reward attacks on isolated and backward pawns
    score += evaluate_attacks_on_weak_pawns(board, color, their_color);

    // 3. Evaluate control of weak squares in opponent's territory
    score += evaluate_weak_squares(board, color, their_color);

    // 4. Detect and reward pins against the opponent
    score += evaluate_pins(pos, color) * PINNED_PIECE_BONUS;

    score
}

fn evaluate_backward_pawns(board: &Board, color: Color) -> i32 {
    let mut backward_pawns = 0;
    let our_pawns = board.by_piece(Piece { role: Role::Pawn, color });
    for pawn_square in our_pawns {
        if is_backward(pawn_square, color, our_pawns) {
            backward_pawns += 1;
        }
    }
    backward_pawns
}

fn is_backward(pawn_square: Square, color: Color, our_pawns: Bitboard) -> bool {
    let file_idx = pawn_square.file() as u32;
    let rank_idx = pawn_square.rank() as u32;

    let behind_rank_idx = if color == Color::White {
        rank_idx.saturating_sub(1)
    } else {
        (rank_idx + 1).min(7)
    };

    let mut adjacent_files = Bitboard::EMPTY;
    if file_idx > 0 {
        adjacent_files |= Bitboard::from_file(File::new(file_idx - 1));
    }
    if file_idx < 7 {
        adjacent_files |= Bitboard::from_file(File::new(file_idx + 1));
    }

    let support_squares = adjacent_files & Bitboard::from_rank(Rank::new(behind_rank_idx));
    (our_pawns & support_squares).is_empty()
}

fn evaluate_attacks_on_weak_pawns(board: &Board, color: Color, their_color: Color) -> i32 {
    let mut score = 0;
    let their_pawns = board.by_piece(Piece { role: Role::Pawn, color: their_color });
    let our_attacks = get_all_attacks(board, color);

    for pawn_square in their_pawns {
        let is_isolated = is_isolated(pawn_square, their_pawns);
        let is_backward = is_backward(pawn_square, their_color, their_pawns);

        if (is_isolated || is_backward) && !(our_attacks & Bitboard::from_square(pawn_square)).is_empty() {
            score += ATTACK_ON_WEAK_PAWN_BONUS;
        }
    }
    score
}

fn is_isolated(pawn_square: Square, pawns: Bitboard) -> bool {
    let file_idx = pawn_square.file() as u32;
    let mut adjacent_files = Bitboard::EMPTY;
    if file_idx > 0 {
        adjacent_files |= Bitboard::from_file(File::new(file_idx - 1));
    }
    if file_idx < 7 {
        adjacent_files |= Bitboard::from_file(File::new(file_idx + 1));
    }
    (pawns & adjacent_files).is_empty()
}

fn evaluate_weak_squares(board: &Board, color: Color, their_color: Color) -> i32 {
    let mut score = 0;
    let their_pawns = board.by_piece(Piece { role: Role::Pawn, color: their_color });
    let our_pieces = board.by_color(color) & !board.pawns(); // Knights, bishops, etc.

    for file in File::ALL {
        for rank in Rank::ALL {
            let square = Square::from_coords(file, rank);
            // A square is weak if no enemy pawn can attack it.
            if attacks::pawn_attacks(color, square).intersect(their_pawns).is_empty() {
                 // If one of our minor/major pieces occupies this weak square, it's an outpost.
                 if !(our_pieces & Bitboard::from_square(square)).is_empty() {
                    // Bonus is higher for squares deeper in their territory.
                    let rank_idx = if color == Color::White { rank as u32 } else { 7 - (rank as u32) };
                    if rank_idx >= 4 { // 5th rank or deeper
                        score += WEAK_SQUARE_CONTROL_BONUS * (1 + rank_idx as i32 - 4);
                    }
                 }
            }
        }
    }
    score
}

fn evaluate_pins(pos: &Chess, color: Color) -> i32 {
    let mut pinned_pieces = 0;
    let board = pos.board();
    let their_king_sq = match board.king_of(!color) {
        Some(sq) => sq,
        None => return 0,
    };

    // Check for pins from our rooks, bishops, and queens
    let our_sliders = board.by_color(color) & (board.rooks() | board.bishops() | board.queens());

    for slider_sq in our_sliders {
        let slider = board.piece_at(slider_sq).unwrap();
        // Check if there's a direct line of attack to their king
        let attacks = get_sliding_attacks(slider.role, slider_sq, board.occupied());
        if !(attacks & Bitboard::from_square(their_king_sq)).is_empty() {
            // There's a potential pin. Check for exactly one piece between slider and king.
            if let Some(pinned_sq) = get_piece_between(slider_sq, their_king_sq, board) {
                // Make sure the piece is theirs
                if let Some(pinned_piece) = board.piece_at(pinned_sq) {
                    if pinned_piece.color == !color {
                        pinned_pieces += 1;
                    }
                }
            }
        }
    }
    pinned_pieces
}

// Helper to get attacks for sliding pieces (Rook, Bishop, Queen)
fn get_sliding_attacks(role: Role, sq: Square, occupied: Bitboard) -> Bitboard {
    match role {
        Role::Bishop => attacks::bishop_attacks(sq, occupied),
        Role::Rook => attacks::rook_attacks(sq, occupied),
        Role::Queen => attacks::queen_attacks(sq, occupied),
        _ => Bitboard::EMPTY,
    }
}

// Helper to find a single piece between two squares on a line or diagonal
fn get_piece_between(sq1: Square, sq2: Square, board: &Board) -> Option<Square> {
    let between_squares = attacks::between(sq1, sq2);
    let pieces_on_line = between_squares & board.occupied();
    if pieces_on_line.count() == 1 {
        return pieces_on_line.into_iter().next();
    }
    None
}


fn get_all_attacks(board: &Board, color: Color) -> Bitboard {
    let mut all_attacks = Bitboard::EMPTY;
    let occupied = board.occupied();
    for sq in board.by_color(color) {
        if let Some(piece) = board.piece_at(sq) {
            all_attacks |= match piece.role {
                Role::Pawn => attacks::pawn_attacks(color, sq),
                Role::Knight => attacks::knight_attacks(sq),
                Role::Bishop => attacks::bishop_attacks(sq, occupied),
                Role::Rook => attacks::rook_attacks(sq, occupied),
                Role::Queen => attacks::queen_attacks(sq, occupied),
                Role::King => attacks::king_attacks(sq),
            };
        }
    }
    all_attacks
}
