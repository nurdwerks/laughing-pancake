// game/mod.rs

use shakmaty::{Chess, Position, Move, Color};
use shakmaty::uci::UciMove;
use shakmaty::san::San;
use rand::Rng;

pub struct GameState {
    pub chess: Chess,
    pgn: String,
}

impl GameState {
    pub fn new() -> Self {
        Self {
            chess: Chess::default(),
            pgn: String::new(),
        }
    }

    pub fn make_move(&mut self, uci_move: &UciMove) -> bool {
        if let Ok(m) = uci_move.to_move(&self.chess) {
            if self.chess.turn() == Color::White {
                self.pgn.push_str(&format!("{}. ", self.chess.fullmoves()));
            }
            let san = San::from_move(&self.chess, m);
            self.pgn.push_str(&san.to_string());
            self.pgn.push(' ');
            self.chess.play_unchecked(m);
            true
        } else {
            false
        }
    }

    pub fn get_legal_moves(&self) -> Vec<Move> {
        self.chess.legal_moves().to_vec()
    }

    pub fn is_game_over(&self) -> bool {
        self.chess.is_game_over()
    }

    pub fn get_pgn(&self) -> &str {
        &self.pgn
    }

    pub fn get_random_move(&self) -> Option<Move> {
        let legal_moves = self.get_legal_moves();
        if legal_moves.is_empty() {
            None
        } else {
            let mut rng = rand::thread_rng();
            let random_index = rng.gen_range(0..legal_moves.len());
            Some(legal_moves[random_index].clone())
        }
    }
}
