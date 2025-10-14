//! Piece-Square Tables (PSTs) for chess evaluation.
//!
//! The values are from the [Chess Programming Wiki](https://www.chessprogramming.org/Simplified_Evaluation_Function).

#![allow(clippy::unusual_byte_groupings)]

type Pst = [[i32; 8]; 8];

const fn flip(pst: &Pst) -> Pst {
    let mut flipped = [[0; 8]; 8];
    let mut i = 0;
    while i < 8 {
        let mut j = 0;
        while j < 8 {
            flipped[i][j] = pst[7 - i][j];
            j += 1;
        }
        i += 1;
    }
    flipped
}

#[rustfmt::skip]
const PAWN_PST_OPENING: Pst = [
    [  0,   0,   0,   0,   0,   0,   0,   0],
    [ 50,  50,  50,  50,  50,  50,  50,  50],
    [ 10,  10,  20,  30,  30,  20,  10,  10],
    [  5,   5,  10,  25,  25,  10,   5,   5],
    [  0,   0,   0,  20,  20,   0,   0,   0],
    [  5,  -5, -10,   0,   0, -10,  -5,   5],
    [  5,  10,  10, -20, -20,  10,  10,   5],
    [  0,   0,   0,   0,   0,   0,   0,   0],
];

#[rustfmt::skip]
const PAWN_PST_ENDGAME: Pst = [
    [  0,   0,   0,   0,   0,   0,   0,   0],
    [ 80,  80,  80,  80,  80,  80,  80,  80],
    [ 50,  50,  50,  50,  50,  50,  50,  50],
    [ 30,  30,  30,  30,  30,  30,  30,  30],
    [ 20,  20,  20,  20,  20,  20,  20,  20],
    [ 10,  10,  10,  10,  10,  10,  10,  10],
    [  5,   5,   5,   5,   5,   5,   5,   5],
    [  0,   0,   0,   0,   0,   0,   0,   0],
];

#[rustfmt::skip]
const KNIGHT_PST_BASE: Pst = [
    [-50, -40, -30, -30, -30, -30, -40, -50],
    [-40, -20,   0,   0,   0,   0, -20, -40],
    [-30,   0,  10,  15,  15,  10,   0, -30],
    [-30,   5,  15,  20,  20,  15,   5, -30],
    [-30,   0,  15,  20,  20,  15,   0, -30],
    [-30,   5,  10,  15,  15,  10,   5, -30],
    [-40, -20,   0,   5,   5,   0, -20, -40],
    [-50, -40, -30, -30, -30, -30, -40, -50],
];

#[rustfmt::skip]
const BISHOP_PST_BASE: Pst = [
    [-20, -10, -10, -10, -10, -10, -10, -20],
    [-10,   0,   0,   0,   0,   0,   0, -10],
    [-10,   0,   5,  10,  10,   5,   0, -10],
    [-10,   5,   5,  10,  10,   5,   5, -10],
    [-10,   0,  10,  10,  10,  10,   0, -10],
    [-10,  10,  10,  10,  10,  10,  10, -10],
    [-10,   5,   0,   0,   0,   0,   5, -10],
    [-20, -10, -10, -10, -10, -10, -10, -20],
];

#[rustfmt::skip]
const ROOK_PST_BASE: Pst = [
    [  0,   0,   0,   0,   0,   0,   0,   0],
    [  5,  10,  10,  10,  10,  10,  10,   5],
    [ -5,   0,   0,   0,   0,   0,   0,  -5],
    [ -5,   0,   0,   0,   0,   0,   0,  -5],
    [ -5,   0,   0,   0,   0,   0,   0,  -5],
    [ -5,   0,   0,   0,   0,   0,   0,  -5],
    [ -5,   0,   0,   0,   0,   0,   0,  -5],
    [  0,   0,   0,   5,   5,   0,   0,   0],
];

#[rustfmt::skip]
const QUEEN_PST_BASE: Pst = [
    [-20, -10, -10,  -5,  -5, -10, -10, -20],
    [-10,   0,   0,   0,   0,   0,   0, -10],
    [-10,   0,   5,   5,   5,   5,   0, -10],
    [ -5,   0,   5,   5,   5,   5,   0,  -5],
    [  0,   0,   5,   5,   5,   5,   0,  -5],
    [-10,   5,   5,   5,   5,   5,   0, -10],
    [-10,   0,   5,   0,   0,   0,   0, -10],
    [-20, -10, -10,  -5,  -5, -10, -10, -20],
];

#[rustfmt::skip]
const KING_PST_OPENING: Pst = [
    [-30, -40, -40, -50, -50, -40, -40, -30],
    [-30, -40, -40, -50, -50, -40, -40, -30],
    [-30, -40, -40, -50, -50, -40, -40, -30],
    [-30, -40, -40, -50, -50, -40, -40, -30],
    [-20, -30, -30, -40, -40, -30, -30, -20],
    [-10, -20, -20, -20, -20, -20, -20, -10],
    [ 20,  20,   0,   0,   0,   0,  20,  20],
    [ 20,  30,  10,   0,   0,  10,  30,  20],
];

#[rustfmt::skip]
const KING_PST_ENDGAME: Pst = [
    [-50, -40, -30, -20, -20, -30, -40, -50],
    [-30, -20, -10,   0,   0, -10, -20, -30],
    [-30, -10,  20,  30,  30,  20, -10, -30],
    [-30, -10,  30,  40,  40,  30, -10, -30],
    [-30, -10,  30,  40,  40,  30, -10, -30],
    [-30, -10,  20,  30,  30,  20, -10, -30],
    [-30, -30,   0,   0,   0,   0, -30, -30],
    [-50, -30, -30, -30, -30, -30, -30, -50],
];

// Flipped PSTs for black pieces
const BLACK_PAWN_PST_OPENING: Pst = flip(&PAWN_PST_OPENING);
const BLACK_PAWN_PST_ENDGAME: Pst = flip(&PAWN_PST_ENDGAME);
const BLACK_KNIGHT_PST_BASE: Pst = flip(&KNIGHT_PST_BASE);
const BLACK_BISHOP_PST_BASE: Pst = flip(&BISHOP_PST_BASE);
const BLACK_ROOK_PST_BASE: Pst = flip(&ROOK_PST_BASE);
const BLACK_QUEEN_PST_BASE: Pst = flip(&QUEEN_PST_BASE);
const BLACK_KING_PST_OPENING: Pst = flip(&KING_PST_OPENING);
const BLACK_KING_PST_ENDGAME: Pst = flip(&KING_PST_ENDGAME);

pub const PAWN_PST: (Pst, Pst) = (PAWN_PST_OPENING, PAWN_PST_ENDGAME);
pub const KNIGHT_PST: (Pst, Pst) = (KNIGHT_PST_BASE, KNIGHT_PST_BASE);
pub const BISHOP_PST: (Pst, Pst) = (BISHOP_PST_BASE, BISHOP_PST_BASE);
pub const ROOK_PST: (Pst, Pst) = (ROOK_PST_BASE, ROOK_PST_BASE);
pub const QUEEN_PST: (Pst, Pst) = (QUEEN_PST_BASE, QUEEN_PST_BASE);
pub const KING_PST: (Pst, Pst) = (KING_PST_OPENING, KING_PST_ENDGAME);

pub const BLACK_PAWN_PST: (Pst, Pst) = (BLACK_PAWN_PST_OPENING, BLACK_PAWN_PST_ENDGAME);
pub const BLACK_KNIGHT_PST: (Pst, Pst) = (BLACK_KNIGHT_PST_BASE, BLACK_KNIGHT_PST_BASE);
pub const BLACK_BISHOP_PST: (Pst, Pst) = (BLACK_BISHOP_PST_BASE, BLACK_BISHOP_PST_BASE);
pub const BLACK_ROOK_PST: (Pst, Pst) = (BLACK_ROOK_PST_BASE, BLACK_ROOK_PST_BASE);
pub const BLACK_QUEEN_PST: (Pst, Pst) = (BLACK_QUEEN_PST_BASE, BLACK_QUEEN_PST_BASE);
pub const BLACK_KING_PST: (Pst, Pst) = (BLACK_KING_PST_OPENING, BLACK_KING_PST_ENDGAME);
