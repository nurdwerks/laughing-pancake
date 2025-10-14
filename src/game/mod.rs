// game/mod.rs

pub mod evaluation;

use pgn_reader::{Reader, Visitor, SanPlus};
use shakmaty::{Chess, Position, Move, Color};
use shakmaty::uci::UciMove;
use shakmaty::san::San;
use rand::Rng;
use shakmaty_syzygy::{Tablebase, Wdl};
use shakmaty::zobrist::{ZobristHash, Zobrist64};
use std::collections::HashMap;
use std::fs::File;
use std::io::BufReader;
use std::ops::ControlFlow;

struct BookBuilder {
    book: HashMap<Zobrist64, Vec<Move>>,
    board: Chess,
}

impl Visitor for BookBuilder {
    type Tags = ();
    type Movetext = ();
    type Output = HashMap<Zobrist64, Vec<Move>>;

    fn begin_tags(&mut self) -> ControlFlow<Self::Output, Self::Tags> {
        ControlFlow::Continue(())
    }

    fn begin_movetext(&mut self, _tags: Self::Tags) -> ControlFlow<Self::Output, Self::Movetext> {
        self.board = Chess::default();
        ControlFlow::Continue(())
    }

    fn san(&mut self, _movetext: &mut Self::Movetext, san_plus: SanPlus) -> ControlFlow<Self::Output> {
        let hash = self.board.zobrist_hash(shakmaty::EnPassantMode::Legal);
        if let Ok(m) = san_plus.san.to_move(&self.board) {
            self.book.entry(hash).or_insert_with(Vec::new).push(m);
            if let Ok(new_board) = self.board.clone().play(m) {
                self.board = new_board;
            } else {
                return ControlFlow::Break(self.book.clone());
            }
        }
        ControlFlow::Continue(())
    }

    fn end_game(&mut self, _movetext: Self::Movetext) -> Self::Output {
        self.book.clone()
    }
}

pub struct GameState {
    pub chess: Chess,
    pgn: String,
    tablebase: Option<Tablebase<Chess>>,
    opening_book: Option<HashMap<Zobrist64, Vec<Move>>>,
}

impl GameState {
    pub fn new(
        tablebase_path: Option<String>,
        opening_book_path: Option<String>,
    ) -> (Self, Option<String>) {
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

        let opening_book = if let Some(path) = opening_book_path {
            match File::open(&path) {
                Ok(file) => {
                    let reader = BufReader::new(file);
                    let mut builder = BookBuilder {
                        book: HashMap::new(),
                        board: Chess::default(),
                    };
                    let mut reader = Reader::new(reader);
                    let _ = reader.read_game(&mut builder);
                    Some(builder.book)
                }
                Err(_) => {
                    warning = Some(format!("Could not load opening book: {}", path));
                    None
                }
            }
        } else {
            None
        };

        (
            Self {
                chess: Chess::default(),
                pgn: String::new(),
                tablebase,
                opening_book,
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
        if let Some(book) = &self.opening_book {
            let hash = self.chess.zobrist_hash(shakmaty::EnPassantMode::Legal);
            if let Some(moves) = book.get(&hash) {
                if !moves.is_empty() {
                    let mut rng = rand::thread_rng();
                    let random_index = rng.gen_range(0..moves.len());
                    return Some(moves[random_index]);
                }
            }
        }

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

        // Fallback to evaluation if no other move is found
        let legal_moves = self.get_legal_moves();
        if legal_moves.is_empty() {
            return None;
        }

        let mut best_score = i32::MIN;
        let mut best_moves = Vec::new();

        for m in legal_moves {
            let mut new_pos = self.chess.clone();
            new_pos.play_unchecked(m);

            // The evaluate function returns the score from the perspective of the current player.
            // Since the move has been made, the board's turn has switched to the opponent.
            // So, we need to negate the score to get it from the perspective of the player making the move.
            let score = -evaluation::evaluate(&new_pos);

            if score > best_score {
                best_score = score;
                best_moves.clear();
                best_moves.push(m);
            } else if score == best_score {
                best_moves.push(m);
            }
        }

        if best_moves.is_empty() {
            None
        } else {
            let mut rng = rand::thread_rng();
            let random_index = rng.gen_range(0..best_moves.len());
            Some(best_moves[random_index].clone())
        }
    }
}
