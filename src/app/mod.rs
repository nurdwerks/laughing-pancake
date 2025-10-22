// app/mod.rs

use crate::{
    constants::NUM_ROUNDS,
    event::{
        ActiveMatchState, ComponentState, Event, SelectionAlgorithm, StsLeaderboardEntry,
        WebsocketState, EVENT_BROKER,
    },
    ga,
    worker,
};
use shakmaty::{fen::Fen, Chess};
use std::{
    collections::HashMap,
    io,
    sync::{Arc, Mutex},
    thread,
    time::{Duration},
};
use sysinfo::System;
use tokio::sync::broadcast;

#[derive(Clone, Default)]
#[cfg_attr(test, allow(dead_code))]
pub struct ActiveMatch {
    pub board: Option<Chess>,
    pub white_player: String,
    pub black_player: String,
    pub san: String,
    pub eval: i32,
    pub material: i32,
}

use sysinfo::{Components};
use std::hash::{Hash, Hasher};

#[cfg_attr(test, allow(dead_code))]
pub struct App {
    should_quit: bool,
    pub error_message: Option<String>,
    // System info
    pub system: System,
    pub components: Components,
    pub cpu_usage: f32,
    pub memory_usage: u64,
    pub total_memory: u64,
    // Evolution state
    event_subscriber: broadcast::Receiver<Event>,
    pub evolution_current_generation: u32,
    pub evolution_current_round: usize,
    pub evolution_matches_completed: usize,
    pub evolution_total_matches: usize,
    pub active_matches: HashMap<usize, ActiveMatch>,
    evolution_thread_handle: Option<thread::JoinHandle<()>>,
    evolution_should_quit: Arc<Mutex<bool>>,
    match_id_counter: Arc<Mutex<usize>>,
    selection_algorithm: SelectionAlgorithm,
    sts_leaderboard: Vec<StsLeaderboardEntry>,
    sts_hash_to_id_map: HashMap<u64, usize>,
    // Websocket state
    git_hash: String,
}

impl App {
    #[cfg_attr(test, allow(dead_code))]
    pub fn new(git_hash: String) -> Self {
        let mut system = System::new_all();
        system.refresh_all();

        Self {
            should_quit: false,
            error_message: None,
            // System info
            system,
            components: Components::new_with_refreshed_list(),
            cpu_usage: 0.0,
            memory_usage: 0,
            total_memory: 0,
            // Evolution state
            event_subscriber: EVENT_BROKER.subscribe(),
            evolution_current_generation: 0,
            evolution_current_round: 0,
            evolution_matches_completed: 0,
            evolution_total_matches: 0,
            active_matches: HashMap::new(),
            evolution_thread_handle: None,
            evolution_should_quit: Arc::new(Mutex::new(false)),
            match_id_counter: Arc::new(Mutex::new(0)),
            selection_algorithm: SelectionAlgorithm::SwissTournament,
            sts_leaderboard: Vec::new(),
            sts_hash_to_id_map: HashMap::new(),
            // Websocket state
            git_hash,
        }
    }

    #[cfg_attr(test, allow(dead_code))]
    pub async fn run_headless(&mut self) -> io::Result<()> {
        self.start_evolution();
        while !self.should_quit {
            self.update_system_stats();
            self.publish_ws_state_update();
            self.handle_app_events().await?;

            // In headless mode, we can sleep for a bit to avoid busy-waiting
            tokio::time::sleep(Duration::from_millis(50)).await;
        }

        if let Some(handle) = self.evolution_thread_handle.take() {
            handle.join().unwrap();
        }

        Ok(())
    }

    #[cfg_attr(test, allow(dead_code))]
    fn update_system_stats(&mut self) {
        self.system.refresh_cpu_all();
        self.system.refresh_memory();
        self.components.refresh(true);
        self.cpu_usage = self.system.global_cpu_usage();
        self.memory_usage = self.system.used_memory();
        self.total_memory = self.system.total_memory();
    }

    #[cfg_attr(test, allow(dead_code))]
    fn publish_ws_state_update(&mut self) {
        let state = self.get_websocket_state();
        EVENT_BROKER.publish(Event::WebsocketStateUpdate(state));
    }

    #[cfg_attr(test, allow(dead_code))]
    async fn handle_app_events(&mut self) -> io::Result<()> {
        while let Ok(update) = self.event_subscriber.try_recv() {
            match update {
                Event::StsModeActive(algo, population) => {
                    self.selection_algorithm = algo;
                    self.sts_leaderboard.clear();
                    self.sts_hash_to_id_map.clear();
                    for individual in population.individuals {
                        let mut hasher = std::collections::hash_map::DefaultHasher::new();
                        individual.config.hash(&mut hasher);
                        let hash = hasher.finish();
                        self.sts_hash_to_id_map.insert(hash, individual.id);
                    }
                }
                Event::TournamentStart(round, total_matches, skipped_matches) => {
                    self.active_matches.clear();
                    self.evolution_current_round = round;
                    self.evolution_total_matches = total_matches;
                    self.evolution_matches_completed = skipped_matches;
                }
                Event::GenerationStarted(gen_index) => {
                    self.evolution_current_generation = gen_index;
                    self.evolution_current_round = 0;
                    self.evolution_matches_completed = 0;
                    self.evolution_total_matches = 0;
                    self.active_matches.clear();
                }
                Event::GenerationComplete(stats) => {
                    let log_message = format!(
                        "Gen {}: {} matches (W:{} B:{} D:{}), ELOs (Top: {:.2}, Avg: {:.2}, Low: {:.2})",
                        stats.generation_index,
                        stats.num_matches,
                        stats.white_wins,
                        stats.black_wins,
                        stats.draws,
                        stats.top_elo,
                        stats.average_elo,
                        stats.lowest_elo
                    );
                    EVENT_BROKER.publish(Event::LogUpdate(log_message));
                }
                Event::MatchStarted(match_id, white_player, black_player) => {
                    let match_state = ActiveMatch {
                        white_player,
                        black_player,
                        ..Default::default()
                    };
                    self.active_matches.insert(match_id, match_state);
                }
                Event::MatchCompleted(match_id, result) => {
                    self.evolution_matches_completed += 1;
                    self.active_matches.remove(&match_id);

                    let white_name = result.white_player_name.replace(".json", "");
                    let black_name = result.black_player_name.replace(".json", "");

                    let white_num = extract_player_number(&white_name);
                    let black_num = extract_player_number(&black_name);

                    let log_message = format!(
                        "M {}: {} vs {} ({})",
                        match_id, white_num, black_num, result.result
                    );
                    EVENT_BROKER.publish(Event::LogUpdate(log_message));
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
                    EVENT_BROKER.publish(Event::LogUpdate(message));
                }
                Event::Panic(msg) => {
                    self.error_message = Some(format!("Evolution thread panicked: {msg}"));
                    self.should_quit = true;
                }
                Event::ForceQuit => {
                    std::process::exit(0);
                }
                Event::ResetSimulation => {
                    println!("Deleting evolution directory...");
                    if let Err(e) = std::fs::remove_dir_all("evolution") {
                        self.error_message = Some(format!("Failed to delete evolution directory: {e}"));
                    } else {
                        println!("Evolution directory deleted.");
                    }

                    println!("Deleting sts_results directory...");
                    if let Err(e) = std::fs::remove_dir_all("sts_results") {
                        self.error_message = Some(format!("Failed to delete sts_results directory: {e}"));
                    } else {
                        println!("sts_results directory deleted.");
                    }
                    std::process::exit(0);
                }
                Event::WebsocketStateUpdate(_) | Event::LogUpdate(_) => {
                    // Ignore, this event is for the web client
                }
                Event::StsUpdate(update) => {
                    if let Some(individual_id) = self.sts_hash_to_id_map.get(&update.config_hash) {
                        let entry = StsLeaderboardEntry {
                            individual_id: *individual_id,
                            progress: update.progress,
                            elo: update.elo,
                        };
                        if let Some(existing_entry) = self
                            .sts_leaderboard
                            .iter_mut()
                            .find(|e| e.individual_id == *individual_id)
                        {
                            *existing_entry = entry;
                        } else {
                            self.sts_leaderboard.push(entry);
                        }
                    }
                }
            }
        }
        Ok(())
    }

    #[cfg_attr(test, allow(dead_code))]
    fn start_evolution(&mut self) {
        let evolution_manager = ga::EvolutionManager::new(
            self.evolution_should_quit.clone(),
            self.match_id_counter.clone(),
        );
        let handle = thread::spawn(move || {
            evolution_manager.run();
        });
        self.evolution_thread_handle = Some(handle);
    }

    #[cfg_attr(test, allow(dead_code))]
    fn get_websocket_state(&self) -> WebsocketState {
        WebsocketState {
            git_hash: self.git_hash.clone(),
            cpu_usage: self.cpu_usage,
            memory_usage: self.memory_usage,
            total_memory: self.total_memory,
            components: self
                .components
                .iter()
                .map(|c| ComponentState {
                    label: c.label().to_string(),
                    temperature: c.temperature().unwrap_or(0.0),
                })
                .collect(),
            evolution_current_generation: self.evolution_current_generation,
            evolution_current_round: self.evolution_current_round,
            evolution_total_rounds: NUM_ROUNDS,
            evolution_matches_completed: self.evolution_matches_completed,
            evolution_total_matches: self.evolution_total_matches,
            active_matches: self
                .active_matches
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
            worker_statuses: worker::WORKER_STATUSES.lock().unwrap().clone(),
            selection_algorithm: self.selection_algorithm.clone(),
            sts_leaderboard: self.sts_leaderboard.clone(),
        }
    }
}

#[cfg_attr(test, allow(dead_code))]
fn extract_player_number(name: &str) -> &str {
    if let Some(pos) = name.rfind(|c: char| !c.is_ascii_digit()) {
        &name[pos + 1..]
    } else {
        name
    }
}