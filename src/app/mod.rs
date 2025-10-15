// app/mod.rs

use crate::config;
use crate::game::{search::SearchConfig, GameState};
use crate::ui;
use crossterm::event::{self, Event, KeyCode};
use ratatui::{prelude::*, Terminal};
use shakmaty::{Color, Move, Outcome, Position, KnownOutcome};
use shakmaty::uci::UciMove;
use std::io;
use std::str::FromStr;
use std::thread;
use std::time::Duration;
use crossbeam_channel::{unbounded, Sender, Receiver};

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
    pub tablebase_path: Option<String>,
    pub opening_book_path: Option<String>,
    // AI configuration state
    pub show_ai_config: bool,
    pub profiles: Vec<String>,
    pub selected_profile_index: usize,
    pub current_search_config: SearchConfig,
    pub selected_config_line: usize,
    // AI search state
    is_ai_searching: bool,
    ai_move_sender: Sender<Move>,
    ai_move_receiver: Receiver<Move>,
}

impl App {
    pub fn new(tablebase_path: Option<String>, opening_book_path: Option<String>) -> Self {
        let (game_state, warning) =
            GameState::new(tablebase_path.clone(), opening_book_path.clone());
        let profiles = config::get_profiles().unwrap_or_else(|_| vec!["default".to_string()]);
        let default_config = SearchConfig::default();
        let (tx, rx) = unbounded();

        // Ensure the default profile exists
        if !profiles.contains(&"default".to_string()) {
            let _ = config::save_profile("default", &default_config);
        }

        Self {
            game_state,
            should_quit: false,
            user_input: String::new(),
            error_message: warning,
            game_mode: GameMode::PlayerVsAi,
            game_result: None,
            tablebase_path,
            opening_book_path,
            show_ai_config: false,
            profiles,
            selected_profile_index: 0,
            current_search_config: default_config,
            selected_config_line: 0,
            is_ai_searching: false,
            ai_move_sender: tx,
            ai_move_receiver: rx,
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

                if is_ai_turn && !self.is_ai_searching {
                    self.is_ai_searching = true;
                    let game_state = self.game_state.clone();
                    let sender = self.ai_move_sender.clone();
                    thread::spawn(move || {
                        if let Some(ai_move) = game_state.get_ai_move() {
                            let _ = sender.send(ai_move);
                        }
                    });
                }

                if let Ok(ai_move) = self.ai_move_receiver.try_recv() {
                    self.is_ai_searching = false;
                    let uci_move = ai_move.to_uci(self.game_state.chess.castles().mode());
                    self.game_state.make_move(&uci_move);
                }
            }
        }
        Ok(())
    }

    fn handle_events(&mut self) -> io::Result<()> {
        if event::poll(Duration::from_millis(50))? {
            if let Event::Key(key) = event::read()? {
                if key.kind == event::KeyEventKind::Press {
                    if self.show_ai_config {
                        self.handle_config_events(key.code);
                    } else {
                        match key.code {
                            KeyCode::Char('q') => {
                                self.should_quit = true;
                            }
                            KeyCode::Char('c') => {
                                self.show_ai_config = true;
                            }
                            KeyCode::Char('s') => {
                                self.game_mode = match self.game_mode {
                                    GameMode::PlayerVsAi => GameMode::AiVsAi,
                                    GameMode::AiVsAi => GameMode::PlayerVsAi,
                                };
                                let (game_state, warning) = GameState::new(
                                    self.tablebase_path.clone(),
                                    self.opening_book_path.clone(),
                                );
                                self.game_state = game_state;
                                self.user_input.clear();
                                self.error_message = warning;
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

    fn handle_config_events(&mut self, key_code: KeyCode) {
        match key_code {
            KeyCode::Char('c') | KeyCode::Esc => {
                self.show_ai_config = false;
            }
            KeyCode::Up => {
                if self.selected_profile_index > 0 {
                    self.selected_profile_index -= 1;
                }
            }
            KeyCode::Down => {
                if self.selected_profile_index < self.profiles.len() - 1 {
                    self.selected_profile_index += 1;
                }
            }
            KeyCode::Enter => {
                let profile_name = &self.profiles[self.selected_profile_index];
                if let Ok(config) = config::load_profile(profile_name) {
                    self.current_search_config = config;
                    self.game_state.search_config = self.current_search_config.clone();
                    self.show_ai_config = false;
                } else {
                    self.error_message = Some(format!("Failed to load profile: {}", profile_name));
                }
            }
            KeyCode::Char('j') => {
                self.selected_config_line = (self.selected_config_line + 1).min(28);
            }
            KeyCode::Char('k') => {
                if self.selected_config_line > 0 {
                    self.selected_config_line -= 1;
                }
            }
            KeyCode::Char('l') => self.modify_config_value(true),
            KeyCode::Char('h') => self.modify_config_value(false),
            KeyCode::Char('s') => {
                let profile_name = &self.profiles[self.selected_profile_index];
                if config::save_profile(profile_name, &self.current_search_config).is_ok() {
                    self.error_message = Some(format!("Profile saved: {}", profile_name));
                } else {
                    self.error_message = Some(format!("Failed to save profile: {}", profile_name));
                }
            }
            _ => {}
        }
    }

    fn modify_config_value(&mut self, increase: bool) {
        let config = &mut self.current_search_config;
        use crate::game::search::SearchAlgorithm;
        match self.selected_config_line {
            0 => {
                config.search_algorithm = match config.search_algorithm {
                    SearchAlgorithm::Pvs => SearchAlgorithm::Mcts,
                    SearchAlgorithm::Mcts => SearchAlgorithm::Pvs,
                };
            }
            1 => config.mcts_simulations = if increase { (config.mcts_simulations + 100).min(10000) } else { (config.mcts_simulations.saturating_sub(100)).max(100) },
            2 => config.use_aspiration_windows = !config.use_aspiration_windows,
            3 => config.use_history_heuristic = !config.use_history_heuristic,
            4 => config.use_killer_moves = !config.use_killer_moves,
            5 => config.use_quiescence_search = !config.use_quiescence_search,
            6 => config.use_pvs = !config.use_pvs,
            7 => config.use_null_move_pruning = !config.use_null_move_pruning,
            8 => config.use_lmr = !config.use_lmr,
            9 => config.use_futility_pruning = !config.use_futility_pruning,
            10 => config.use_delta_pruning = !config.use_delta_pruning,
            11 => config.pawn_structure_weight = if increase { (config.pawn_structure_weight + 10).min(200) } else { (config.pawn_structure_weight - 10).max(0) },
            12 => config.piece_mobility_weight = if increase { (config.piece_mobility_weight + 10).min(200) } else { (config.piece_mobility_weight - 10).max(0) },
            13 => config.king_safety_weight = if increase { (config.king_safety_weight + 10).min(200) } else { (config.king_safety_weight - 10).max(0) },
            14 => config.piece_development_weight = if increase { (config.piece_development_weight + 10).min(200) } else { (config.piece_development_weight - 10).max(0) },
            15 => config.rook_placement_weight = if increase { (config.rook_placement_weight + 10).min(200) } else { (config.rook_placement_weight - 10).max(0) },
            16 => config.bishop_placement_weight = if increase { (config.bishop_placement_weight + 10).min(200) } else { (config.bishop_placement_weight - 10).max(0) },
            17 => config.knight_placement_weight = if increase { (config.knight_placement_weight + 10).min(200) } else { (config.knight_placement_weight - 10).max(0) },
            18 => config.passed_pawn_weight = if increase { (config.passed_pawn_weight + 10).min(200) } else { (config.passed_pawn_weight - 10).max(0) },
            19 => config.isolated_pawn_weight = if increase { (config.isolated_pawn_weight + 10).min(200) } else { (config.isolated_pawn_weight - 10).max(0) },
            20 => config.doubled_pawn_weight = if increase { (config.doubled_pawn_weight + 10).min(200) } else { (config.doubled_pawn_weight - 10).max(0) },
            21 => config.bishop_pair_weight = if increase { (config.bishop_pair_weight + 10).min(200) } else { (config.bishop_pair_weight - 10).max(0) },
            22 => config.pawn_chain_weight = if increase { (config.pawn_chain_weight + 10).min(200) } else { (config.pawn_chain_weight - 10).max(0) },
            23 => config.ram_weight = if increase { (config.ram_weight + 10).min(200) } else { (config.ram_weight - 10).max(0) },
            24 => config.candidate_passed_pawn_weight = if increase { (config.candidate_passed_pawn_weight + 10).min(200) } else { (config.candidate_passed_pawn_weight - 10).max(0) },
            25 => config.king_pawn_shield_weight = if increase { (config.king_pawn_shield_weight + 10).min(200) } else { (config.king_pawn_shield_weight - 10).max(0) },
            26 => config.king_open_file_penalty = if increase { (config.king_open_file_penalty + 10).min(200) } else { (config.king_open_file_penalty - 10).max(0) },
            27 => config.king_attackers_weight = if increase { (config.king_attackers_weight + 10).min(200) } else { (config.king_attackers_weight - 10).max(0) },
            28 => config.threat_analysis_weight = if increase { (config.threat_analysis_weight + 10).min(200) } else { (config.threat_analysis_weight - 10).max(0) },
            _ => {}
        }
    }
}
