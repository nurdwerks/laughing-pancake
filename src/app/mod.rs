// app/mod.rs

use crate::game::GameState;
use crate::ui;
use crossterm::event::{self, Event, KeyCode};
use ratatui::{prelude::*, Terminal};
use shakmaty::{Color, Outcome, Position, KnownOutcome};
use shakmaty::uci::UciMove;
use std::io;
use std::str::FromStr;
use std::thread;
use std::time::Duration;

#[derive(Clone, Copy, PartialEq)]
pub enum GameMode {
    PlayerVsAi,
    AiVsAi,
}

pub struct App {
    pub game_state: GameState,
    should_quit: bool,
    pub user_input: String,
    pub error_message: Option<String>,
    pub game_mode: GameMode,
    pub game_result: Option<String>,
}

impl App {
    pub fn new() -> Self {
        Self {
            game_state: GameState::new(),
            should_quit: false,
            user_input: String::new(),
            error_message: None,
            game_mode: GameMode::PlayerVsAi,
            game_result: None,
        }
    }

    pub fn run<B: Backend>(&mut self, terminal: &mut Terminal<B>) -> io::Result<()> {
        while !self.should_quit {
            terminal.draw(|f| ui::draw(f, &self))?;
            self.handle_events()?;

            if self.game_state.is_game_over() {
                if self.game_result.is_none() {
                    self.determine_game_result();
                }
            } else {
                let turn = self.game_state.chess.turn();
                let is_ai_turn = match self.game_mode {
                    GameMode::PlayerVsAi => turn == Color::Black,
                    GameMode::AiVsAi => true,
                };

                if is_ai_turn {
                    if let Some(ai_move) = self.game_state.get_random_move() {
                        let uci_move = ai_move.to_uci(self.game_state.chess.castles().mode());
                        self.game_state.make_move(&uci_move);
                        thread::sleep(Duration::from_millis(500));
                    }
                }
            }
        }
        Ok(())
    }

    fn handle_events(&mut self) -> io::Result<()> {
        if event::poll(Duration::from_millis(50))? {
            if let Event::Key(key) = event::read()? {
                if key.kind == event::KeyEventKind::Press {
                    match key.code {
                        KeyCode::Char('q') => {
                            self.should_quit = true;
                        }
                        KeyCode::Char('s') => {
                            self.game_mode = match self.game_mode {
                                GameMode::PlayerVsAi => GameMode::AiVsAi,
                                GameMode::AiVsAi => GameMode::PlayerVsAi,
                            };
                            self.game_state = GameState::new();
                            self.user_input.clear();
                            self.error_message = None;
                            self.game_result = None;
                        }
                        KeyCode::Char(c) => {
                            if self.game_mode == GameMode::PlayerVsAi {
                                self.user_input.push(c);
                                self.error_message = None;
                            }
                        }
                        KeyCode::Backspace => {
                            if self.game_mode == GameMode::PlayerVsAi {
                                self.user_input.pop();
                            }
                        }
                        KeyCode::Enter => {
                            if self.game_mode == GameMode::PlayerVsAi {
                                self.handle_move_input();
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
        Ok(())
    }

    fn handle_move_input(&mut self) {
        let input = self.user_input.trim();
        if self.game_state.chess.turn() == Color::White {
            match UciMove::from_str(input) {
                Ok(uci_move) => {
                    if self.game_state.make_move(&uci_move) {
                        self.error_message = None;
                    } else {
                        self.error_message = Some("Illegal move".to_string());
                    }
                }
                Err(_) => {
                    self.error_message = Some("Invalid UCI format".to_string());
                }
            }
        } else {
            self.error_message = Some("Not your turn".to_string());
        }
        self.user_input.clear();
    }

    fn determine_game_result(&mut self) {
        let outcome = self.game_state.chess.outcome();
        self.game_result = match outcome {
            Outcome::Known(KnownOutcome::Draw) => Some("Draw".to_string()),
            Outcome::Known(KnownOutcome::Decisive { winner, .. }) => {
                Some(format!("Checkmate! {:?} wins", winner))
            }
            Outcome::Unknown => None,
        };
    }
}
