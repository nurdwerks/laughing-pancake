// app/mod.rs

use crate::{ga::{self, EvolutionUpdate}};
use crate::ui;
use crossterm::event::{self, Event, KeyCode};
use lazy_static::lazy_static;
use std::sync::{Mutex, Arc};
use ratatui::{prelude::*, Terminal, widgets::ListState};
use shakmaty::{Chess};
use std::io;
use std::thread;
use std::time::{Duration, Instant};
use crossbeam_channel::{unbounded, Sender, Receiver};
use sysinfo::System;

lazy_static! {
    pub static ref TUI_WRITER_SENDER: Mutex<Option<Sender<String>>> = Mutex::new(None);
}

pub struct TuiMakeWriter;

impl TuiMakeWriter {
    pub fn new() -> Self {
        TuiMakeWriter
    }
}

impl io::Write for TuiMakeWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let msg = String::from_utf8_lossy(buf).to_string();
        if let Some(sender) = TUI_WRITER_SENDER.lock().unwrap().as_ref() {
            sender.send(msg).unwrap();
        }
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[derive(Clone)]
pub struct Worker {
    pub id: u64,
    pub name: String,
    pub start_time: Instant,
}

pub struct App {
    should_quit: bool,
    pub error_message: Option<String>,
    // System info
    system: System,
    pub cpu_usage: f32,
    pub memory_usage: u64,
    pub total_memory: u64,
    // Evolution state
    evolution_sender: Sender<ga::EvolutionUpdate>,
    pub evolution_receiver: Receiver<ga::EvolutionUpdate>,
    pub evolution_log: Vec<String>,
    pub evolution_log_state: ListState,
    pub evolution_current_generation: u32,
    pub evolution_matches_completed: usize,
    pub evolution_total_matches: usize,
    pub evolution_current_match_board: Option<Chess>,
    pub evolution_current_match_eval: i32,
    pub evolution_current_match_san: String,
    pub evolution_material_advantage: i32,
    evolution_thread_handle: Option<thread::JoinHandle<()>>,
    pub evolution_white_player: String,
    pub evolution_black_player: String,
    pub evolution_workers: Arc<Mutex<Vec<Worker>>>,
    log_receiver: Receiver<String>,
}

impl App {
    pub fn new(_tablebase_path: Option<String>, _opening_book_path: Option<String>, log_receiver: Receiver<String>) -> Self {
        let (evo_tx, evo_rx) = unbounded();
        let mut system = System::new_all();
        system.refresh_all();

        Self {
            should_quit: false,
            error_message: None,
            // System info
            system,
            cpu_usage: 0.0,
            memory_usage: 0,
            total_memory: 0,
            // Evolution state
            evolution_sender: evo_tx,
            evolution_receiver: evo_rx,
            evolution_log: Vec::new(),
            evolution_log_state: ListState::default(),
            evolution_current_generation: 0,
            evolution_matches_completed: 0,
            evolution_total_matches: 0,
            evolution_current_match_board: None,
            evolution_current_match_eval: 0,
            evolution_current_match_san: "".to_string(),
            evolution_material_advantage: 0,
            evolution_thread_handle: None,
            evolution_white_player: "".to_string(),
            evolution_black_player: "".to_string(),
            evolution_workers: Arc::new(Mutex::new(Vec::new())),
            log_receiver,
        }
    }

    pub fn run<B: Backend>(&mut self, terminal: &mut Terminal<B>) -> io::Result<()> {
        self.start_evolution();
        while !self.should_quit {
            self.update_system_stats();
            terminal.draw(|f| ui::draw(f, self))?;
            self.handle_events()?;
            self.handle_evolution_updates();
            self.handle_log_updates();

            if let Some(handle) = &self.evolution_thread_handle {
                if handle.is_finished() {
                    self.should_quit = true;
                }
            }
        }
        Ok(())
    }

    fn update_system_stats(&mut self) {
        self.system.refresh_cpu_all();
        self.system.refresh_memory();
        self.cpu_usage = self.system.global_cpu_usage();
        self.memory_usage = self.system.used_memory();
        self.total_memory = self.system.total_memory();
    }

    fn handle_events(&mut self) -> io::Result<()> {
        if event::poll(Duration::from_millis(50))? {
            if let Event::Key(key) = event::read()? {
                if key.kind == event::KeyEventKind::Press {
                    match key.code {
                        KeyCode::Char('q') => {
                            self.should_quit = true;
                        }
                        KeyCode::Up => {
                            let new_selection = self.evolution_log_state.selected().unwrap_or(0).saturating_sub(1);
                            self.evolution_log_state.select(Some(new_selection));
                        }
                        KeyCode::Down => {
                            if !self.evolution_log.is_empty() {
                                let new_selection = self.evolution_log_state.selected().unwrap_or(0).saturating_add(1).min(self.evolution_log.len() - 1);
                                self.evolution_log_state.select(Some(new_selection));
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
        Ok(())
    }

    fn start_evolution(&mut self) {
        let evolution_manager = ga::EvolutionManager::new(self.evolution_sender.clone(), self.evolution_workers.clone());
        let handle = thread::spawn(move || {
            evolution_manager.run();
        });
        self.evolution_thread_handle = Some(handle);
    }

    fn handle_log_updates(&mut self) {
        while let Ok(log_msg) = self.log_receiver.try_recv() {
            self.evolution_log.push(log_msg.trim().to_string());
            self.autoscroll_log();
        }
    }

    fn handle_evolution_updates(&mut self) {
        while let Ok(update) = self.evolution_receiver.try_recv() {
            match update {
                EvolutionUpdate::GenerationStarted(gen_index) => {
                    self.evolution_current_generation = gen_index;
                    self.evolution_matches_completed = 0;
                    self.evolution_total_matches = 9900; // POPULATION_SIZE * (POPULATION_SIZE - 1)
                }
                EvolutionUpdate::MatchStarted(white_player, black_player) => {
                    self.evolution_white_player = white_player;
                    self.evolution_black_player = black_player;
                }
                EvolutionUpdate::MatchCompleted(game_match) => {
                    self.evolution_matches_completed += 1;

                    let result_str = match game_match.result.as_str() {
                        "1-0" => format!("White wins ({})", game_match.white_player_name),
                        "0-1" => format!("Black wins ({})", game_match.black_player_name),
                        "1/2-1/2" => "Draw".to_string(),
                        _ => "Unknown result".to_string(),
                    };
                    let log_message = format!("Match complete: {}.", result_str);
                    self.evolution_log.push(log_message);
                    self.autoscroll_log();

                    self.evolution_current_match_san.clear();
                    self.evolution_material_advantage = 0;
                }
                EvolutionUpdate::ThinkingUpdate(_pv, eval) => {
                    self.evolution_current_match_eval = eval;
                }
                EvolutionUpdate::MovePlayed(san, material, board) => {
                    self.evolution_current_match_san.push_str(&format!("{san} "));
                    self.evolution_material_advantage = material;
                    self.evolution_current_match_board = Some(board);
                }
                EvolutionUpdate::StatusUpdate(message) => {
                    self.evolution_log.push(message);
                    self.autoscroll_log();
                }
                EvolutionUpdate::Panic(msg) => {
                    self.error_message = Some(format!("Evolution thread panicked: {msg}"));
                    self.should_quit = true;
                }
            }
        }
    }

    fn autoscroll_log(&mut self) {
        let log_len = self.evolution_log.len();
        if log_len > 100 { // Keep the log at a max of 100 entries
            self.evolution_log.drain(0..log_len - 100);
        }
        // Autoscroll to the bottom of the log
        self.evolution_log_state.select(Some(self.evolution_log.len().saturating_sub(1)));
    }

    pub fn get_config_for_player(&self, player_name: &str) -> Option<crate::game::search::SearchConfig> {
        if player_name.is_empty() {
            return None;
        }
        let path = std::path::Path::new("evolution")
            .join(format!("generation_{}", self.evolution_current_generation))
            .join(player_name);

        if let Ok(file_content) = std::fs::read_to_string(path) {
            serde_json::from_str(&file_content).ok()
        } else {
            None
        }
    }
}
