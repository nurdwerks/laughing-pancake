// app/mod.rs

use crate::{
    event::{
        ActiveMatchState, ComponentState, CpuState, Event, WebsocketState, WorkerState,
        EVENT_BROKER,
    },
    ga, ui,
};
use crossterm::event::{self, KeyCode};
use ratatui::{prelude::*, widgets::ListState, Terminal};
use shakmaty::{fen::Fen, Chess};
use std::{
    collections::HashMap,
    io,
    sync::{Arc, Mutex},
    thread,
    time::{Duration, Instant},
};
use sysinfo::System;
use tokio::sync::broadcast;

#[derive(Clone)]
pub struct Worker {
    pub id: u64,
    pub name: String,
    pub start_time: Instant,
}

#[derive(Clone, Default)]
pub struct ActiveMatch {
    pub board: Option<Chess>,
    pub white_player: String,
    pub black_player: String,
    pub san: String,
    pub eval: i32,
    pub material: i32,
}

use sysinfo::{Components};

pub struct App {
    should_quit: bool,
    pub graceful_quit: bool,
    pub error_message: Option<String>,
    // System info
    pub system: System,
    pub components: Components,
    pub cpu_usage: f32,
    pub memory_usage: u64,
    pub total_memory: u64,
    // Evolution state
    event_subscriber: broadcast::Receiver<Event>,
    pub evolution_log: Vec<String>,
    pub evolution_log_state: ListState,
    pub evolution_current_generation: u32,
    pub evolution_matches_completed: usize,
    pub evolution_total_matches: usize,
    pub active_matches: HashMap<usize, ActiveMatch>,
    evolution_thread_handle: Option<thread::JoinHandle<()>>,
    evolution_should_quit: Arc<Mutex<bool>>,
    pub evolution_workers: Arc<Mutex<Vec<Worker>>>,
    // Websocket state
    last_ws_update: Instant,
    git_hash: String,
}

impl App {
    pub fn new(git_hash: String) -> Self {
        let mut system = System::new_all();
        system.refresh_all();

        Self {
            should_quit: false,
            graceful_quit: false,
            error_message: None,
            // System info
            system,
            components: Components::new_with_refreshed_list(),
            cpu_usage: 0.0,
            memory_usage: 0,
            total_memory: 0,
            // Evolution state
            event_subscriber: EVENT_BROKER.subscribe(),
            evolution_log: Vec::new(),
            evolution_log_state: ListState::default(),
            evolution_current_generation: 0,
            evolution_matches_completed: 0,
            evolution_total_matches: 0,
            active_matches: HashMap::new(),
            evolution_thread_handle: None,
            evolution_should_quit: Arc::new(Mutex::new(false)),
            evolution_workers: Arc::new(Mutex::new(Vec::new())),
            // Websocket state
            last_ws_update: Instant::now(),
            git_hash,
        }
    }

    pub async fn run<B: Backend>(&mut self, terminal: &mut Terminal<B>) -> io::Result<()> {
        self.start_evolution();
        while !self.should_quit {
            self.update_system_stats();
            self.publish_ws_state_update();
            terminal.draw(|f| ui::draw(f, self))?;
            self.handle_events().await?;

            if self.graceful_quit && self.active_matches.is_empty() {
                // The graceful quit flag is set, and no matches are running.
                // Now we can signal the main loop to quit and wait for the thread to save.
                self.should_quit = true;
                if let Some(handle) = self.evolution_thread_handle.take() {
                    // Wait for the evolution thread to finish, which includes saving the final state.
                    handle.join().unwrap();
                }
            }
        }
        Ok(())
    }

    fn update_system_stats(&mut self) {
        self.system.refresh_cpu_all();
        self.system.refresh_memory();
        self.components.refresh(true);
        self.cpu_usage = self.system.global_cpu_usage();
        self.memory_usage = self.system.used_memory();
        self.total_memory = self.system.total_memory();
    }

    fn publish_ws_state_update(&mut self) {
        if self.last_ws_update.elapsed() < Duration::from_millis(500) {
            return;
        }

        let workers_arc = self.evolution_workers.clone();
        let evolution_log = self.evolution_log.clone();
        let active_matches = self.active_matches.clone();
        let state_clone = self.clone_state_for_websocket();

        tokio::spawn(async move {
            let workers = workers_arc.lock().unwrap();
            let state = WebsocketState {
                git_hash: state_clone.git_hash,
                graceful_shutdown: state_clone.graceful_shutdown,
                cpu_usage: state_clone.cpu_usage,
                memory_usage: state_clone.memory_usage,
                total_memory: state_clone.total_memory,
                cpus: state_clone.cpus,
                components: state_clone.components,
                evolution_log,
                evolution_current_generation: state_clone.evolution_current_generation,
                evolution_matches_completed: state_clone.evolution_matches_completed,
                evolution_total_matches: state_clone.evolution_total_matches,
                active_matches: active_matches
                    .iter()
                    .filter_map(|(id, m)| {
                        m.board.as_ref().map(|board| {
                            let fen: Fen = Fen::from_position(board, shakmaty::EnPassantMode::Legal);
                            (
                                *id,
                                ActiveMatchState {
                                    board: fen.to_string(),
                                    white_player: m.white_player.clone(),
                                    black_player: m.black_player.clone(),
                                    san: m.san.clone(),
                                    eval: m.eval,
                                    material: m.material,
                                },
                            )
                        })
                    })
                    .collect(),
                evolution_workers: workers
                    .iter()
                    .map(|w| WorkerState {
                        id: w.id,
                        name: w.name.clone(),
                        elapsed_time: w.start_time.elapsed().as_secs_f64(),
                    })
                    .collect(),
            };
            EVENT_BROKER.publish(Event::WebsocketStateUpdate(state));
        });

        self.last_ws_update = Instant::now();
    }

    async fn handle_events(&mut self) -> io::Result<()> {
        // Handle keyboard events
        if event::poll(Duration::from_millis(50))? {
            if let crossterm::event::Event::Key(key) = event::read()? {
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

        // Handle evolution events
        while let Ok(update) = self.event_subscriber.try_recv() {
            match update {
                Event::TournamentStart(total_matches, skipped_matches) => {
                    self.evolution_total_matches = total_matches;
                    self.evolution_matches_completed = skipped_matches;
                }
                Event::GenerationStarted(gen_index) => {
                    self.evolution_current_generation = gen_index;
                    self.evolution_matches_completed = 0;
                    self.evolution_total_matches = 0;
                    self.active_matches.clear();
                }
                Event::MatchStarted(match_id, white_player, black_player) => {
                    let match_state = ActiveMatch {
                        white_player,
                        black_player,
                        ..Default::default()
                    };
                    self.active_matches.insert(match_id, match_state);
                }
                Event::MatchCompleted(match_id, game_match) => {
                    self.evolution_matches_completed += 1;
                    self.active_matches.remove(&match_id);

                    let result_str = match game_match.result.as_str() {
                        "1-0" => format!("White wins ({})", game_match.white_player_name.replace(".json", "")),
                        "0-1" => format!("Black wins ({})", game_match.black_player_name.replace(".json", "")),
                        "1/2-1/2" => "Draw".to_string(),
                        _ => "Unknown result".to_string(),
                    };
                    let log_message = format!("[Match {match_id}] Complete: {result_str}.");
                    self.evolution_log.push(log_message);
                    self.autoscroll_log();
                }
                Event::ThinkingUpdate(match_id, _pv, eval) => {
                    if let Some(match_state) = self.active_matches.get_mut(&match_id) {
                        match_state.eval = eval;
                    }
                }
                Event::MovePlayed(match_id, san, material, board) => {
                    if let Some(match_state) = self.active_matches.get_mut(&match_id) {
                        match_state.san.push_str(&format!("{san} "));
                        match_state.material = material;
                        match_state.board = Some(board);
                    }
                }
                Event::StatusUpdate(message) => {
                    self.evolution_log.push(message);
                    self.autoscroll_log();
                }
                Event::Panic(msg) => {
                    self.error_message = Some(format!("Evolution thread panicked: {msg}"));
                    self.should_quit = true;
                }
                Event::RequestQuit => {
                    *self.evolution_should_quit.lock().unwrap() = true;
                    self.graceful_quit = true;
                    EVENT_BROKER.publish(Event::StatusUpdate("Graceful shutdown initiated. Waiting for current matches to complete...".to_string()));
                }
                Event::ForceQuit => {
                    *self.evolution_should_quit.lock().unwrap() = true;
                    self.should_quit = true;
                }
                Event::WebsocketStateUpdate(_) => {
                    // Ignore, this event is for the web client
                }
            }
        }
        Ok(())
    }

    fn start_evolution(&mut self) {
        let evolution_manager = ga::EvolutionManager::new(
            self.evolution_workers.clone(),
            self.evolution_should_quit.clone(),
        );
        let handle = thread::spawn(move || {
            evolution_manager.run();
        });
        self.evolution_thread_handle = Some(handle);
    }

    fn autoscroll_log(&mut self) {
        let log_len = self.evolution_log.len();
        if log_len > 100 { // Keep the log at a max of 100 entries
            self.evolution_log.drain(0..log_len - 100);
        }
        // Autoscroll to the bottom of the log
        self.evolution_log_state.select(Some(self.evolution_log.len().saturating_sub(1)));
    }

    fn clone_state_for_websocket(&self) -> WebsocketState {
        WebsocketState {
            git_hash: self.git_hash.clone(),
            graceful_shutdown: self.graceful_quit,
            cpu_usage: self.cpu_usage,
            memory_usage: self.memory_usage,
            total_memory: self.total_memory,
            cpus: self
                .system
                .cpus()
                .iter()
                .map(|c| CpuState {
                    usage: c.cpu_usage(),
                })
                .collect(),
            components: self
                .components
                .iter()
                .map(|c| ComponentState {
                    label: c.label().to_string(),
                    temperature: c.temperature().unwrap_or(0.0),
                })
                .collect(),
            evolution_log: Vec::new(), // This is cloned separately
            evolution_current_generation: self.evolution_current_generation,
            evolution_matches_completed: self.evolution_matches_completed,
            evolution_total_matches: self.evolution_total_matches,
            active_matches: HashMap::new(), // This is cloned separately
            evolution_workers: Vec::new(),   // This is cloned separately
        }
    }
}