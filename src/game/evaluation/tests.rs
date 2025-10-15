//! Unit tests for the new evaluation terms.

use super::*;
use crate::game::search::SearchConfig;
use shakmaty::{fen::Fen, CastlingMode, Chess, Position};

#[test]
fn test_game_phase_starting_position() {
    let pos = Chess::default();
    assert_eq!(game_phase(pos.board()), 256);
}

#[test]
fn test_game_phase_endgame() {
    let fen: Fen = "8/4k3/8/8/8/8/4K3/8 w - - 0 1".parse().unwrap();
    let pos: Chess = fen.into_position(CastlingMode::Standard).unwrap();
    assert_eq!(game_phase(pos.board()), 0);
}

#[test]
fn test_evaluate_starting_position() {
    let pos = Chess::default();
    let config = SearchConfig::default();
    let score = evaluate(&pos, &config);
    // The score is not exactly 0 because of the PSTs and other eval terms.
    // A wider range might be needed. Let's check for a reasonable score.
    assert!(score > -50 && score < 50);
}

#[test]
fn test_evaluate_white_advantage() {
    let fen: Fen = "4k3/8/8/8/8/8/8/4K2Q w - - 0 1".parse().unwrap();
    let pos: Chess = fen.into_position(CastlingMode::Standard).unwrap();
    let config = SearchConfig::default();
    let score = evaluate(&pos, &config);
    assert!(score > 850);
}

#[test]
fn test_evaluate_black_advantage() {
    let fen: Fen = "4k2q/8/8/8/8/8/8/4K3 w - - 0 1".parse().unwrap();
    let pos: Chess = fen.into_position(CastlingMode::Standard).unwrap();
    let config = SearchConfig::default();
    let score = evaluate(&pos, &config);
    assert!(score < -850);
}

#[test]
fn test_evaluate_black_advantage_black_to_move() {
    let fen: Fen = "4k2q/8/8/8/8/8/8/4K3 b - - 0 1".parse().unwrap();
    let pos: Chess = fen.into_position(CastlingMode::Standard).unwrap();
    let config = SearchConfig::default();
    let score = evaluate(&pos, &config);
    assert!(score > 850);
}

#[test]
fn test_rook_open_file() {
    // White rook on an open file (e-file)
    let fen: Fen = "4k3/8/8/8/8/8/8/4K2R w K - 0 1".parse().unwrap();
    let pos: Chess = fen.into_position(CastlingMode::Standard).unwrap();
    let score = rooks::evaluate(pos.board(), shakmaty::Color::White);
    assert_eq!(score, 20); // OPEN_FILE_BONUS
}

#[test]
fn test_rook_semi_open_file() {
    // White rook on a semi-open file (h-file), only blocked by a black pawn
    let fen: Fen = "4k3/8/8/8/8/7p/8/4K2R w K - 0 1".parse().unwrap();
    let pos: Chess = fen.into_position(CastlingMode::Standard).unwrap();
    let score = rooks::evaluate(pos.board(), shakmaty::Color::White);
    assert_eq!(score, 10); // SEMI_OPEN_FILE_BONUS
}

#[test]
fn test_rook_on_seventh_rank() {
    // White rook on the 7th rank
    let fen: Fen = "4k3/7R/8/8/8/8/8/4K3 w - - 0 1".parse().unwrap();
    let pos: Chess = fen.into_position(CastlingMode::Standard).unwrap();
    let score = rooks::evaluate(pos.board(), shakmaty::Color::White);
    // 20 for open file + 25 for 7th rank
    assert_eq!(score, 45);
}

#[test]
fn test_bishop_pair() {
    // White has a bishop pair
    let fen: Fen = "4k3/8/8/8/8/8/B7/B3K3 w - - 0 1".parse().unwrap();
    let pos: Chess = fen.into_position(CastlingMode::Standard).unwrap();
    let config = SearchConfig::default();
    let score = bishops::evaluate(pos.board(), shakmaty::Color::White, &config);
    assert_eq!(score, config.bishop_pair_weight / 100);
}

#[test]
fn test_pawn_structure_evaluation() {
    // White: Doubled pawns on b-file, isolated pawn on d-file, passed pawn on f-file
    let fen: Fen = "4k3/8/8/5p2/3P4/1P6/1P2K3/8 w - - 0 1".parse().unwrap();
    let pos: Chess = fen.into_position(CastlingMode::Standard).unwrap();
    let config = SearchConfig::default();
    let score = pawn_structure::evaluate(pos.board(), shakmaty::Color::White, &config);

    let expected_doubled_penalty = -config.doubled_pawn_weight / 100;
    let expected_isolated_penalty = -config.isolated_pawn_weight / 100;
    let expected_passed_bonus = config.passed_pawn_weight / 100;
    let expected_score = expected_doubled_penalty + expected_isolated_penalty + expected_passed_bonus;

    assert_eq!(score, expected_score);
}

#[test]
fn test_bad_bishop() {
    // White light-squared bishop is blocked by central pawns on light squares
    let fen: Fen = "4k3/8/8/8/3p4/3P4/B7/4K3 w - - 0 1".parse().unwrap();
    let pos: Chess = fen.into_position(CastlingMode::Standard).unwrap();
    let config = SearchConfig::default();
    let score = bishops::evaluate(pos.board(), shakmaty::Color::White, &config);
    // The pawn on d3 is on a light square, same as the bishop on a2.
    // This is just one pawn, so the penalty is -10.
    // Bishop pair bonus is not applied, so the total score is just the penalty.
    assert_eq!(score, -10); // BAD_BISHOP_PENALTY
}

#[test]
fn test_knight_outpost() {
    // White knight on d5 is an outpost.
    // It's on the 5th rank, supported by a pawn on c4, and cannot be attacked by black pawns.
    // White knight on d5 is an outpost.
    // It's on the 5th rank, supported by a pawn on c4, and cannot be attacked by black pawns.
    let fen: Fen = "4k3/8/8/3N4/2P5/8/8/4K3 w - - 0 1".parse().unwrap();
    let pos: Chess = fen.into_position(CastlingMode::Standard).unwrap();
    let _score = knights::evaluate(pos.board(), shakmaty::Color::White);
    // The test was failing because I was asserting the wrong score.
    // The knight is on an outpost (30) and is centralized (10), so the score should be 40.
    assert_eq!(_score, 40);
}

#[test]
fn test_knight_centralization() {
    // White knight on e4
    let fen: Fen = "4k3/8/8/8/4N3/8/8/4K3 w - - 0 1".parse().unwrap();
    let pos: Chess = fen.into_position(CastlingMode::Standard).unwrap();
    let score = knights::evaluate(pos.board(), shakmaty::Color::White);
    assert_eq!(score, 10); // CENTRALIZATION_BONUS
}
