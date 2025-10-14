// game/mod.rs

use shakmaty::{Chess, Position, Move, Color};
use shakmaty::uci::UciMove;
use shakmaty::san::San;
use rand::Rng;
use shakmaty_syzygy::{Tablebase, Wdl};

pub struct GameState {
    pub chess: Chess,
    pgn: String,
    tablebase: Option<Tablebase<Chess>>,
}

impl GameState {
    pub fn new(tablebase_path: Option<String>) -> (Self, Option<String>) {
        let mut tablebase = None;
        let mut warning = None;

        if let Some(path) = tablebase_path {
            let mut tb = Tablebase::new();
            if tb.add_directory(&path).is_err() {
                warning = Some(format!("Invalid tablebase path: {}", path));
            } else if tb.max_pieces() == 0 {
                warning = Some(format!("No tablebase files found in: {}", path));
            } else {
                tablebase = Some(tb);
            }
        }

        (
            Self {
                chess: Chess::default(),
                pgn: String::new(),
                tablebase,
            },
            warning,
        )
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

    pub fn get_ai_move(&self) -> Option<Move> {
        if let Some(tb) = &self.tablebase {
            let legal_moves = self.get_legal_moves();
            let mut winning_moves = Vec::new();
            let mut drawing_moves = Vec::new();
            let mut losing_moves = Vec::new();

            for m in legal_moves {
                let mut new_pos = self.chess.clone();
                new_pos.play_unchecked(m);
                if let Ok(wdl) = tb.probe_wdl_after_zeroing(&new_pos) {
                    let inverted_wdl = -wdl;
                    match inverted_wdl {
                        Wdl::Win => winning_moves.push(m),
                        Wdl::Draw => drawing_moves.push(m),
                        _ => losing_moves.push(m),
                    }
                } else {
                    losing_moves.push(m);
                }
            }

            if !winning_moves.is_empty() {
                let mut rng = rand::thread_rng();
                let random_index = rng.gen_range(0..winning_moves.len());
                return Some(winning_moves[random_index]);
            }
            if !drawing_moves.is_empty() {
                let mut rng = rand::thread_rng();
                let random_index = rng.gen_range(0..drawing_moves.len());
                return Some(drawing_moves[random_index]);
            }
            if !losing_moves.is_empty() {
                let mut rng = rand::thread_rng();
                let random_index = rng.gen_range(0..losing_moves.len());
                return Some(losing_moves[random_index]);
            }
        }

        // Fallback to random move if tablebase is not available or doesn't provide a move
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
