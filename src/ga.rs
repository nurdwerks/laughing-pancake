use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use rand::Rng;
use rand::distributions::Distribution;
use shakmaty::{Chess, Position};
use shakmaty::san::SanPlus;
use serde::{Deserialize, Serialize};

use crate::app::Worker;
use crate::game::search::{self, SearchConfig, SearchAlgorithm, PvsSearcher, Searcher, evaluation_cache::EvaluationCache};
use crossbeam_channel::{Sender, unbounded};

const EVOLUTION_DIR: &str = "evolution";
const POPULATION_SIZE: usize = 100;
const MUTATION_CHANCE: f64 = 0.05; // 5% chance for each parameter to mutate

#[derive(Debug, Clone)]
pub enum EvolutionUpdate {
    TournamentStart(usize, usize), // Total matches, skipped matches
    GenerationStarted(u32),
    MatchStarted(usize, String, String), // Match index, White player name, Black player name
    MatchCompleted(usize, Match), // Match index, Match
    ThinkingUpdate(usize, String, i32),  // Match index, Thinking message, evaluation
    MovePlayed(usize, String, i32, Chess), // Match index, SAN of the move, material difference, new board position
    StatusUpdate(String),
    Panic(String),
}
/// Manages evaluation caches for all players in the tournament.
/// Caches are created on-demand and automatically destroyed when no longer in use.
struct CacheManager {
    caches: Arc<Mutex<HashMap<String, Arc<Mutex<EvaluationCache>>>>>,
    usage_count: Arc<Mutex<HashMap<String, usize>>>,
}

impl CacheManager {
    fn new() -> Self {
        Self {
            caches: Arc::new(Mutex::new(HashMap::new())),
            usage_count: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Gets a cache for a specific player.
    /// This will create a cache if one doesn't exist.
    /// It returns a `CacheGuard`, which will automatically decrement the usage count
    /// when it goes out of scope.
    fn get_cache_for_player(&self, player_name: &str) -> CacheGuard {
        let mut caches = self.caches.lock().unwrap();
        let mut usage_count = self.usage_count.lock().unwrap();

        let cache = caches
            .entry(player_name.to_string())
            .or_insert_with(|| Arc::new(Mutex::new(EvaluationCache::new())))
            .clone();

        *usage_count.entry(player_name.to_string()).or_insert(0) += 1;

        CacheGuard {
            player_name: player_name.to_string(),
            cache: cache.clone(),
            cache_manager: self.clone(),
        }
    }

    /// Called by `CacheGuard` when it's dropped.
    /// Decrements the usage count for a player's cache and removes it if the count is zero.
    fn release_cache(&self, player_name: &str) {
        let mut caches = self.caches.lock().unwrap();
        let mut usage_count = self.usage_count.lock().unwrap();

        if let Some(count) = usage_count.get_mut(player_name) {
            *count -= 1;
            if *count == 0 {
                usage_count.remove(player_name);
                caches.remove(player_name);
            }
        }
    }
}

impl Clone for CacheManager {
    fn clone(&self) -> Self {
        Self {
            caches: self.caches.clone(),
            usage_count: self.usage_count.clone(),
        }
    }
}

/// A guard that holds a reference to a player's cache.
/// When this guard is dropped, it notifies the `CacheManager` to decrement the usage count.
struct CacheGuard {
    player_name: String,
    cache: Arc<Mutex<EvaluationCache>>,
    cache_manager: CacheManager,
}

impl Drop for CacheGuard {
    fn drop(&mut self) {
        self.cache_manager.release_cache(&self.player_name);
    }
}


/// Manages the evolution process in a background thread.
#[derive(Clone)]
pub struct EvolutionManager {
    update_sender: Sender<EvolutionUpdate>,
    workers: Arc<Mutex<Vec<Worker>>>,
}

impl EvolutionManager {
    pub fn new(update_sender: Sender<EvolutionUpdate>, workers: Arc<Mutex<Vec<Worker>>>) -> Self {
        Self { update_sender, workers }
    }

    fn send_status(&self, message: String) -> Result<(), ()> {
        if self.update_sender.send(EvolutionUpdate::StatusUpdate(message)).is_err() {
            return Err(());
        }
        Ok(())
    }

    pub fn run(&self) {
        let result = std::panic::catch_unwind(|| {
            if self.run_internal().is_err() {
                // The receiver has been dropped, so the thread can exit.
            }
        });

        if let Err(e) = result {
            let panic_msg = if let Some(s) = e.downcast_ref::<String>() {
                s.clone()
            } else if let Some(s) = e.downcast_ref::<&str>() {
                s.to_string()
            } else {
                "Unknown panic!".to_string()
            };
            let _ = self.update_sender.send(EvolutionUpdate::Panic(panic_msg));
        }
    }

    fn run_internal(&self) -> Result<(), ()> {
        self.send_status("Starting evolution process".to_string())?;
        let mut generation_index = find_latest_complete_generation().unwrap_or(0);
        if generation_index > 0 {
             self.send_status(format!("Resuming from last completed generation: {generation_index}."))?;
        }

        // Special handling for first ever run.
        if generation_index == 0 && !Path::new(EVOLUTION_DIR).join("generation_0/individual_99.json").exists() {
            self.send_status("No existing population found. Generating initial population for Generation 0.".to_string())?;
            let generation_dir = setup_directories(0);
            generate_initial_population(&generation_dir);
        }


        loop {
            self.send_status(format!("--- Starting Generation {generation_index} ---"))?;
            self.update_sender.send(EvolutionUpdate::GenerationStarted(generation_index)).map_err(|_| ())?;
            let generation_dir = setup_directories(generation_index);

            self.send_status(format!("Loading population for generation {generation_index}..."))?;
            let mut population = Population::load(&generation_dir);
            self.send_status(format!("Loaded {} individuals.", population.individuals.len()))?;

            let mut generation = self.load_or_create_generation(generation_index, &population)?;
            let cache_manager = CacheManager::new();

            self.run_tournament(&mut population, &mut generation, &cache_manager)?;

            let next_generation_dir = setup_directories(generation_index + 1);
            self.evolve_population(&population, &next_generation_dir)?;
            self.send_status(format!("--- Generation {generation_index} Complete ---"))?;
            generation_index += 1;
        }
    }

    /// Loads a generation from a file, or creates a new one if it doesn't exist or is corrupt.
    fn load_or_create_generation(&self, generation_index: u32, population: &Population) -> Result<Generation, ()> {
        let file_path = Path::new(EVOLUTION_DIR)
            .join(format!("generation_{generation_index}.json"));

        if file_path.exists() {
            let json = fs::read_to_string(&file_path);
            if let Ok(json_content) = json {
                let generation: Result<Generation, _> = serde_json::from_str(&json_content);
                if let Ok(gen) = generation {
                    self.send_status(format!("Successfully loaded existing match data for generation {generation_index}."))?;
                    return Ok(gen);
                } else {
                    self.send_status(format!("Warning: Found corrupt generation file at {file_path:?}. Starting generation from scratch."))?;
                }
            } else {
                 self.send_status(format!("Warning: Could not read generation file at {file_path:?}. Starting generation from scratch."))?;
            }
        }

        self.send_status(format!("No existing match data found for generation {generation_index}. Creating new tournament."))?;
        let games = generate_pairings(population);
        let matches = games.into_iter().map(|game| Match {
            white_player_name: format!("individual_{}.json", game.white_player_id),
            black_player_name: format!("individual_{}.json", game.black_player_id),
            status: "pending".to_string(),
            result: "".to_string(),
            san: "".to_string(),
        }).collect();

        Ok(Generation {
            generation_index,
            matches,
        })
    }

    /// Takes a completed tournament population and evolves it to create the next generation.
    fn evolve_population(&self, population: &Population, next_generation_dir: &Path) -> Result<(), ()> {
        self.send_status("\nEvolving to the next generation...".to_string())?;

        // 1. Selection: Find the top 5 individuals
        let mut sorted_individuals = population.individuals.iter().collect::<Vec<_>>();
        sorted_individuals.sort_by_key(|i| i.score);
        sorted_individuals.reverse(); // Highest score first
        let elites = &sorted_individuals[0..5];

        self.send_status("Top 5 Elites (by score):".to_string())?;
        for (i, elite) in elites.iter().enumerate() {
            self.send_status(format!(
                "{}. Individual {} (Score: {})",
                i + 1,
                elite.id,
                elite.score
            ))?;
        }

        let mut rng = rand::thread_rng();

        // 2. Elitism: Copy the top 5 to the next generation
        for (i, elite) in elites.iter().enumerate().take(5) {
            let elite_config_path = next_generation_dir.join(format!("individual_{i}.json"));
            let json = serde_json::to_string_pretty(&elite.config).expect("Failed to serialize elite config");
            fs::write(elite_config_path, json).expect("Failed to write elite config file");
        }

        // 3. Breeding & 4. Mutation: Create the remaining 95 individuals
        for i in 5..POPULATION_SIZE {
            // Select two random parents from the elite pool
            let parent1 = elites[rng.gen_range(0..elites.len())];
            let parent2 = elites[rng.gen_range(0..elites.len())];

            // Create the child by crossing over parameters
            let mut child_config = crossover(&parent1.config, &parent2.config, &mut rng);

            // Mutate the child
            mutate(&mut child_config, &mut rng);

            let child_config_path = next_generation_dir.join(format!("individual_{i}.json"));
            let json = serde_json::to_string_pretty(&child_config).expect("Failed to serialize child config");
            fs::write(child_config_path, json).expect("Failed to write child config file");
        }
        self.send_status(format!("Generated and saved {POPULATION_SIZE} new individuals for the next generation."))?;
        Ok(())
    }

    /// Runs all the games in the tournament, saving progress after each game.
    fn run_tournament(&self, population: &mut Population, generation: &mut Generation, cache_manager: &CacheManager) -> Result<(), ()> {
        self.send_status(format!("Running tournament for generation {}", generation.generation_index))?;

        let matches_to_play: Vec<(usize, Match)> = generation.matches.iter().cloned().enumerate()
            .filter(|(_, m)| m.status != "completed")
            .collect();
        let total_matches = generation.matches.len();
        let skipped_matches = total_matches - matches_to_play.len();
        self.send_status(format!("Skipping {} already completed games.", skipped_matches))?;
        self.update_sender.send(EvolutionUpdate::TournamentStart(total_matches, skipped_matches)).map_err(|_| ())?;

        let (results_tx, results_rx) = unbounded();
        let (jobs_tx, jobs_rx) = unbounded::<(usize, Match)>();
        let population_arc = Arc::new(population.clone());

        // Spawn a pool of worker threads
        const NUM_WORKERS: usize = 3;
        let mut worker_handles = Vec::new();
        for _ in 0..NUM_WORKERS {
            let jobs_rx_clone = jobs_rx.clone();
            let results_tx_clone = results_tx.clone();
            let population_clone = Arc::clone(&population_arc);
            let cache_manager_clone = cache_manager.clone();
            let sender_clone = self.update_sender.clone();
            let self_clone = self.clone();

            let handle = std::thread::spawn(move || {
                while let Ok((match_index, game_match)) = jobs_rx_clone.recv() {
                    let white_player_name = game_match.white_player_name.clone();
                    let black_player_name = game_match.black_player_name.clone();

                    // The guards will be dropped at the end of the loop, releasing the caches.
                    let white_cache_guard = cache_manager_clone.get_cache_for_player(&white_player_name);
                    let black_cache_guard = cache_manager_clone.get_cache_for_player(&black_player_name);

                    let white_id = parse_id_from_name(&white_player_name);
                    let black_id = parse_id_from_name(&black_player_name);
                    let white_config = &population_clone.individuals[white_id].config;
                    let black_config = &population_clone.individuals[black_id].config;

                    let _ = sender_clone.send(EvolutionUpdate::MatchStarted(match_index, white_player_name, black_player_name));
                    if let Ok((result, san)) = self_clone.play_game(match_index, white_config, black_config, &white_cache_guard, &black_cache_guard) {
                        if results_tx_clone.send((match_index, result, san)).is_err() {
                            // Main thread has likely shut down, so we can exit.
                            break;
                        }
                    }
                }
            });
            worker_handles.push(handle);
        }

        // Send all jobs to the workers
        for (match_index, game_match) in matches_to_play.clone() {
            if jobs_tx.send((match_index, game_match)).is_err() {
                // This would happen if all worker threads panicked and the channel is closed.
                break;
            }
        }
        drop(jobs_tx); // Close the job channel to signal workers to exit when done

        // Drop the original results sender. The loop below will end when all worker threads
        // have dropped their sender clones.
        drop(results_tx);

        // Process results as they come in
        for (match_index, result, san) in results_rx {
            let mut current_match = generation.matches[match_index].clone();
            current_match.san = san;
            current_match.status = "completed".to_string();

            let white_id = parse_id_from_name(&current_match.white_player_name);
            let black_id = parse_id_from_name(&current_match.black_player_name);

            match result {
                GameResult::WhiteWin => {
                    population.individuals[white_id].score += 1;
                    population.individuals[black_id].score -= 1;
                    current_match.result = "1-0".to_string();
                }
                GameResult::BlackWin => {
                    population.individuals[white_id].score -= 1;
                    population.individuals[black_id].score += 1;
                    current_match.result = "0-1".to_string();
                }
                GameResult::Draw => {
                    current_match.result = "1/2-1/2".to_string();
                }
            }
            generation.matches[match_index] = current_match.clone();
            save_generation(generation);
            self.update_sender.send(EvolutionUpdate::MatchCompleted(match_index, current_match)).map_err(|_| ()).unwrap();
        }

        // Wait for all worker threads to finish
        for handle in worker_handles {
            handle.join().unwrap();
        }

        // Print final tournament results
        self.send_status("\nTournament Results:".to_string())?;
        for individual in &population.individuals {
            self.send_status(format!(
                "Individual {}: Score={}",
                individual.id, individual.score
            ))?;
        }
        Ok(())
    }

    /// Simulates a single game between two AI configurations.
    fn play_game(
        &self,
        match_id: usize,
        white_config: &SearchConfig,
        black_config: &SearchConfig,
        white_cache_guard: &CacheGuard,
        black_cache_guard: &CacheGuard,
    ) -> Result<(GameResult, String), ()> {
        let mut pos = Chess::default();
        let mut sans = Vec::new();

        let mut white_searcher: Box<dyn Searcher> = if white_config.search_algorithm == SearchAlgorithm::Pvs {
            Box::new(PvsSearcher::with_shared_cache(white_cache_guard.cache.clone()))
        } else {
            Box::new(search::mcts::MctsSearcher::new())
        };

        let mut black_searcher: Box<dyn Searcher> = if black_config.search_algorithm == SearchAlgorithm::Pvs {
            Box::new(PvsSearcher::with_shared_cache(black_cache_guard.cache.clone()))
        } else {
            Box::new(search::mcts::MctsSearcher::new())
        };

        let mut game_result_override = None;
        while !pos.is_game_over() {
            // End the game in a draw after 60 moves (120 half-moves/plies).
            if sans.len() >= 120 {
                game_result_override = Some(GameResult::Draw);
                break;
            }

            let (config, searcher) = if pos.turn().is_white() {
                (white_config.clone(), &mut white_searcher)
            } else {
                (black_config.clone(), &mut black_searcher)
            };
            let current_pos = pos.clone();

            let (search_result_tx, search_result_rx) = crossbeam_channel::unbounded();

            let thinking_msg = format!("AI is thinking for {:?}...", current_pos.turn());
            self.update_sender.send(EvolutionUpdate::ThinkingUpdate(match_id, thinking_msg, 0)).map_err(|_| ())?;

            let workers = self.workers.clone();
            let update_sender = self.update_sender.clone();
            crossbeam_utils::thread::scope(|s| {
                s.spawn(|_| {
                    let search_result = searcher.search(&current_pos, config.search_depth, &config, Some(workers), Some(update_sender));
                    search_result_tx.send(search_result).unwrap();
                });

                crossbeam_channel::select! {
                    recv(search_result_rx) -> msg => {
                        if let Ok((best_move, eval, _final_tree)) = msg {
                            let _ = self.update_sender.send(EvolutionUpdate::ThinkingUpdate(match_id, format!("AI finished thinking for {:?}...", current_pos.turn()), eval));
                            if let Some(m) = best_move {
                                let san = SanPlus::from_move(pos.clone(), m);
                                sans.push(san);
                                pos.play_unchecked(m);

                                let material_diff = calculate_material_difference(&pos);
                                let last_san = sans.last().map(|s| s.to_string()).unwrap_or_default();
                                if self.update_sender.send(EvolutionUpdate::MovePlayed(match_id, last_san, material_diff, pos.clone())).is_err() {
                                    // The error will be handled by the outer loop's break condition.
                                }
                            }
                        }
                    }
                }
            }).unwrap();
        }

        let result = if let Some(res) = game_result_override {
            res
        } else {
            let outcome = pos.outcome();
            match outcome.winner() {
                Some(shakmaty::Color::White) => GameResult::WhiteWin,
                Some(shakmaty::Color::Black) => GameResult::BlackWin,
                None => GameResult::Draw,
            }
        };

        let mut pgn = String::new();
        for (i, san) in sans.iter().enumerate() {
            if i % 2 == 0 {
                pgn.push_str(&format!("{}. ", i / 2 + 1));
            }
            pgn.push_str(&format!("{san} "));
        }

        Ok((result, pgn))
    }
}

fn calculate_material_difference(pos: &Chess) -> i32 {
    let board = pos.board();
    let mut white_material = 0;
    let mut black_material = 0;

    for square in shakmaty::Square::ALL {
        if let Some(piece) = board.piece_at(square) {
            let value = match piece.role {
                shakmaty::Role::Pawn => 100,
                shakmaty::Role::Knight => 300,
                shakmaty::Role::Bishop => 320,
                shakmaty::Role::Rook => 500,
                shakmaty::Role::Queen => 900,
                shakmaty::Role::King => 0,
            };
            if piece.color.is_white() {
                white_material += value;
            } else {
                black_material += value;
            }
        }
    }
    white_material - black_material
}

/// Represents a single AI candidate in the population.
#[derive(Clone)]
pub struct Individual {
    pub id: usize,
    pub config: SearchConfig,
    pub score: i32,
}

/// Represents a collection of individuals for a single generation.
#[derive(Clone)]
pub struct Population {
    pub individuals: Vec<Individual>,
}

impl Population {
    /// Loads a population from a generation directory.
    pub fn load(generation_dir: &Path) -> Self {
        let mut individuals = Vec::new();
        for i in 0..POPULATION_SIZE {
            let file_path = generation_dir.join(format!("individual_{i}.json"));
            let json = fs::read_to_string(file_path).expect("Failed to read config file");
            let config: SearchConfig = serde_json::from_str(&json).expect("Failed to deserialize config");

            individuals.push(Individual {
                id: i,
                config,
                score: 0,
            });
        }
        Self { individuals }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Match {
    pub white_player_name: String,
    pub black_player_name: String,
    pub status: String, // "pending", "completed"
    pub result: String, // "1-0", "0-1", "1/2-1/2", ""
    pub san: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Generation {
    pub generation_index: u32,
    pub matches: Vec<Match>,
}

/// Saves the current state of a generation to a JSON file.
pub fn save_generation(generation: &Generation) {
    let file_path = Path::new(EVOLUTION_DIR)
        .join(format!("generation_{}.json", generation.generation_index));
    let json = serde_json::to_string_pretty(generation).expect("Failed to serialize generation state");
    fs::write(file_path, json).expect("Failed to write generation state file");
}

/// The main entry point for the evolutionary algorithm.
/// Represents a single game to be played between two individuals.
struct Game {
    white_player_id: usize,
    black_player_id: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum GameResult {
    WhiteWin,
    BlackWin,
    Draw,
}

/// Helper to extract the individual's ID from its filename.
fn parse_id_from_name(name: &str) -> usize {
    name.strip_prefix("individual_")
        .and_then(|s| s.strip_suffix(".json"))
        .and_then(|s| s.parse::<usize>().ok())
        .expect("Failed to parse individual ID from filename")
}

/// Creates a new SearchConfig by randomly selecting parameters from two parents.
fn crossover(p1: &SearchConfig, p2: &SearchConfig, rng: &mut impl Rng) -> SearchConfig {
    SearchConfig {
        search_depth: (if rng.gen_bool(0.5) { p1.search_depth } else { p2.search_depth }).clamp(3, 5),
        search_algorithm: SearchAlgorithm::Pvs,
        use_aspiration_windows: if rng.gen_bool(0.5) { p1.use_aspiration_windows } else { p2.use_aspiration_windows },
        use_history_heuristic: if rng.gen_bool(0.5) { p1.use_history_heuristic } else { p2.use_history_heuristic },
        use_killer_moves: if rng.gen_bool(0.5) { p1.use_killer_moves } else { p2.use_killer_moves },
        mcts_simulations: if rng.gen_bool(0.5) { p1.mcts_simulations } else { p2.mcts_simulations },
        use_quiescence_search: if rng.gen_bool(0.5) { p1.use_quiescence_search } else { p2.use_quiescence_search },
        use_pvs: if rng.gen_bool(0.5) { p1.use_pvs } else { p2.use_pvs },
        use_null_move_pruning: if rng.gen_bool(0.5) { p1.use_null_move_pruning } else { p2.use_null_move_pruning },
        use_lmr: if rng.gen_bool(0.5) { p1.use_lmr } else { p2.use_lmr },
        use_futility_pruning: if rng.gen_bool(0.5) { p1.use_futility_pruning } else { p2.use_futility_pruning },
        use_delta_pruning: if rng.gen_bool(0.5) { p1.use_delta_pruning } else { p2.use_delta_pruning },
        pawn_structure_weight: if rng.gen_bool(0.5) { p1.pawn_structure_weight } else { p2.pawn_structure_weight },
        piece_mobility_weight: if rng.gen_bool(0.5) { p1.piece_mobility_weight } else { p2.piece_mobility_weight },
        king_safety_weight: if rng.gen_bool(0.5) { p1.king_safety_weight } else { p2.king_safety_weight },
        piece_development_weight: if rng.gen_bool(0.5) { p1.piece_development_weight } else { p2.piece_development_weight },
        rook_placement_weight: if rng.gen_bool(0.5) { p1.rook_placement_weight } else { p2.rook_placement_weight },
        bishop_placement_weight: if rng.gen_bool(0.5) { p1.bishop_placement_weight } else { p2.bishop_placement_weight },
        knight_placement_weight: if rng.gen_bool(0.5) { p1.knight_placement_weight } else { p2.knight_placement_weight },
        passed_pawn_weight: if rng.gen_bool(0.5) { p1.passed_pawn_weight } else { p2.passed_pawn_weight },
        isolated_pawn_weight: if rng.gen_bool(0.5) { p1.isolated_pawn_weight } else { p2.isolated_pawn_weight },
        doubled_pawn_weight: if rng.gen_bool(0.5) { p1.doubled_pawn_weight } else { p2.doubled_pawn_weight },
        bishop_pair_weight: if rng.gen_bool(0.5) { p1.bishop_pair_weight } else { p2.bishop_pair_weight },
        pawn_chain_weight: if rng.gen_bool(0.5) { p1.pawn_chain_weight } else { p2.pawn_chain_weight },
        ram_weight: if rng.gen_bool(0.5) { p1.ram_weight } else { p2.ram_weight },
        candidate_passed_pawn_weight: if rng.gen_bool(0.5) { p1.candidate_passed_pawn_weight } else { p2.candidate_passed_pawn_weight },
        king_pawn_shield_weight: if rng.gen_bool(0.5) { p1.king_pawn_shield_weight } else { p2.king_pawn_shield_weight },
        king_open_file_penalty: if rng.gen_bool(0.5) { p1.king_open_file_penalty } else { p2.king_open_file_penalty },
        king_attackers_weight: if rng.gen_bool(0.5) { p1.king_attackers_weight } else { p2.king_attackers_weight },
        threat_analysis_weight: if rng.gen_bool(0.5) { p1.threat_analysis_weight } else { p2.threat_analysis_weight },
        tempo_bonus_weight: if rng.gen_bool(0.5) { p1.tempo_bonus_weight } else { p2.tempo_bonus_weight },
        space_evaluation_weight: if rng.gen_bool(0.5) { p1.space_evaluation_weight } else { p2.space_evaluation_weight },
        initiative_evaluation_weight: if rng.gen_bool(0.5) { p1.initiative_evaluation_weight } else { p2.initiative_evaluation_weight },
    }
}

/// Applies mutation to a SearchConfig.
fn mutate(config: &mut SearchConfig, rng: &mut impl Rng) {
    if rng.gen_bool(MUTATION_CHANCE) {
        if rng.gen_bool(0.5) {
            config.search_depth = config.search_depth.saturating_add(1);
        } else {
            config.search_depth = config.search_depth.saturating_sub(1);
        }
        config.search_depth = config.search_depth.clamp(3, 5);
    }
    // Mutate booleans with a 3% chance
    if rng.gen_bool(0.03) { config.use_aspiration_windows = !config.use_aspiration_windows; }
    if rng.gen_bool(0.03) { config.use_history_heuristic = !config.use_history_heuristic; }
    if rng.gen_bool(0.03) { config.use_killer_moves = !config.use_killer_moves; }
    if rng.gen_bool(0.03) { config.use_quiescence_search = !config.use_quiescence_search; }
    if rng.gen_bool(0.03) { config.use_pvs = !config.use_pvs; }
    if rng.gen_bool(0.03) { config.use_null_move_pruning = !config.use_null_move_pruning; }
    if rng.gen_bool(0.03) { config.use_lmr = !config.use_lmr; }
    if rng.gen_bool(0.03) { config.use_futility_pruning = !config.use_futility_pruning; }
    if rng.gen_bool(0.03) { config.use_delta_pruning = !config.use_delta_pruning; }

    // Mutate numeric values individually
    config.mcts_simulations = mutate_numeric(config.mcts_simulations as i32, rng) as u32;
    config.pawn_structure_weight = mutate_numeric(config.pawn_structure_weight, rng);
    config.piece_mobility_weight = mutate_numeric(config.piece_mobility_weight, rng);
    config.king_safety_weight = mutate_numeric(config.king_safety_weight, rng);
    config.piece_development_weight = mutate_numeric(config.piece_development_weight, rng);
    config.rook_placement_weight = mutate_numeric(config.rook_placement_weight, rng);
    config.bishop_placement_weight = mutate_numeric(config.bishop_placement_weight, rng);
    config.knight_placement_weight = mutate_numeric(config.knight_placement_weight, rng);
    config.passed_pawn_weight = mutate_numeric(config.passed_pawn_weight, rng);
    config.isolated_pawn_weight = mutate_numeric(config.isolated_pawn_weight, rng);
    config.doubled_pawn_weight = mutate_numeric(config.doubled_pawn_weight, rng);
    config.bishop_pair_weight = mutate_numeric(config.bishop_pair_weight, rng);
    config.pawn_chain_weight = mutate_numeric(config.pawn_chain_weight, rng);
    config.ram_weight = mutate_numeric(config.ram_weight, rng);
    config.candidate_passed_pawn_weight = mutate_numeric(config.candidate_passed_pawn_weight, rng);
    config.king_pawn_shield_weight = mutate_numeric(config.king_pawn_shield_weight, rng);
    config.king_open_file_penalty = mutate_numeric(config.king_open_file_penalty, rng);
    config.king_attackers_weight = mutate_numeric(config.king_attackers_weight, rng);
    config.threat_analysis_weight = mutate_numeric(config.threat_analysis_weight, rng);
    config.tempo_bonus_weight = mutate_numeric(config.tempo_bonus_weight, rng);
    config.space_evaluation_weight = mutate_numeric(config.space_evaluation_weight, rng);
    config.initiative_evaluation_weight = mutate_numeric(config.initiative_evaluation_weight, rng);
}

/// Decides if a mutation should occur and, if so, by how much.
fn mutate_numeric(value: i32, rng: &mut impl Rng) -> i32 {
    if !rng.gen_bool(MUTATION_CHANCE) {
        return value; // No mutation
    }

    // "A 1% change should occur only 25% of the time and it should scale so that at 5% change only occurs at 1% of the time."
    // This implies a distribution of probabilities for the magnitude.
    // Let's use a simple linear interpolation for the probabilities in between.
    // 1% -> 25, 2% -> 19, 3% -> 13, 4% -> 7, 5% -> 1. Total weight = 65.
    let choices = [
        (0.01, 25), // 1% magnitude, ~38.5% chance
        (0.02, 19), // 2% magnitude, ~29.2% chance
        (0.03, 13), // 3% magnitude, ~20.0% chance
        (0.04, 7),  // 4% magnitude, ~10.8% chance
        (0.05, 1),  // 5% magnitude, ~1.5% chance
    ];
    // The user's request is a bit ambiguous, a linear scale is a reasonable interpretation.
    // A 1% change (weight 25) should be 25 times more likely than a 5% change (weight 1).
    let dist = rand::distributions::WeightedIndex::new(choices.iter().map(|item| item.1)).unwrap();
    let change_factor = choices[dist.sample(rng)].0;

    let change = (value as f64 * change_factor).round() as i32;
    let new_value = if rng.gen_bool(0.5) {
        value.saturating_add(change)
    } else {
        value.saturating_sub(change)
    };
    new_value.max(0) // Ensure weights don't go negative
}

/// Finds the index of the latest fully completed generation directory.
fn find_latest_complete_generation() -> Option<u32> {
    if !Path::new(EVOLUTION_DIR).exists() {
        return None;
    }

    fs::read_dir(EVOLUTION_DIR)
        .ok()?
        .filter_map(|entry| {
            let entry = entry.ok()?;
            if entry.file_type().ok()?.is_dir() {
                let dir_name = entry.file_name();
                let dir_str = dir_name.to_str()?;
                if dir_str.starts_with("generation_") {
                    let generation_index = dir_str.strip_prefix("generation_")?.parse::<u32>().ok()?;

                    // Check if the generation is complete by looking for the last individual file.
                    let last_individual_path = entry.path().join(format!("individual_{}.json", POPULATION_SIZE - 1));
                    if last_individual_path.exists() {
                        return Some(generation_index);
                    }
                }
            }
            None
        })
        .max()
}

/// Generates all pairings for a round-robin tournament where each player plays every other player twice.
fn generate_pairings(population: &Population) -> Vec<Game> {
    let mut games = Vec::new();
    for i in 0..population.individuals.len() {
        for j in 0..population.individuals.len() {
            if i == j {
                continue;
            }
            games.push(Game {
                white_player_id: population.individuals[i].id,
                black_player_id: population.individuals[j].id,
            });
        }
    }
    games
}

/// Creates the necessary directories for storing evolution data for a specific generation.
fn setup_directories(generation_index: u32) -> PathBuf {
    if !Path::new(EVOLUTION_DIR).exists() {
        fs::create_dir(EVOLUTION_DIR).expect("Failed to create evolution directory");
    }

    let generation_dir = PathBuf::from(EVOLUTION_DIR).join(format!("generation_{generation_index}"));
    if !generation_dir.exists() {
        fs::create_dir(&generation_dir).expect("Failed to create generation directory");
    }
    generation_dir
}

/// Generates the initial population with random variations from the default config.
fn generate_initial_population(generation_dir: &Path) {
    let mut rng = rand::thread_rng();

    for i in 0..POPULATION_SIZE {
        let mut config = SearchConfig::default();
        let default_config = SearchConfig::default(); // for reference values

        config.search_depth = rng.gen_range(3..=5);

        // Randomize booleans
        config.use_aspiration_windows = rng.gen_bool(0.5);
        config.use_history_heuristic = rng.gen_bool(0.5);
        config.use_killer_moves = rng.gen_bool(0.5);
        config.use_quiescence_search = rng.gen_bool(0.5);
        config.use_pvs = rng.gen_bool(0.5);
        config.use_null_move_pruning = rng.gen_bool(0.5);
        config.use_lmr = rng.gen_bool(0.5);
        config.use_futility_pruning = rng.gen_bool(0.5);
        config.use_delta_pruning = rng.gen_bool(0.5);

        // Randomize enum
        config.search_algorithm = SearchAlgorithm::Pvs;

        // Randomize numeric values with +/- 50% variance
        config.mcts_simulations = vary_numeric(default_config.mcts_simulations as i32, &mut rng) as u32;
        config.pawn_structure_weight = vary_numeric(default_config.pawn_structure_weight, &mut rng);
        config.piece_mobility_weight = vary_numeric(default_config.piece_mobility_weight, &mut rng);
        config.king_safety_weight = vary_numeric(default_config.king_safety_weight, &mut rng);
        config.piece_development_weight = vary_numeric(default_config.piece_development_weight, &mut rng);
        config.rook_placement_weight = vary_numeric(default_config.rook_placement_weight, &mut rng);
        config.bishop_placement_weight = vary_numeric(default_config.bishop_placement_weight, &mut rng);
        config.knight_placement_weight = vary_numeric(default_config.knight_placement_weight, &mut rng);
        config.passed_pawn_weight = vary_numeric(default_config.passed_pawn_weight, &mut rng);
        config.isolated_pawn_weight = vary_numeric(default_config.isolated_pawn_weight, &mut rng);
        config.doubled_pawn_weight = vary_numeric(default_config.doubled_pawn_weight, &mut rng);
        config.bishop_pair_weight = vary_numeric(default_config.bishop_pair_weight, &mut rng);
        config.pawn_chain_weight = vary_numeric(default_config.pawn_chain_weight, &mut rng);
        config.ram_weight = vary_numeric(default_config.ram_weight, &mut rng);
        config.candidate_passed_pawn_weight = vary_numeric(default_config.candidate_passed_pawn_weight, &mut rng);
        config.king_pawn_shield_weight = vary_numeric(default_config.king_pawn_shield_weight, &mut rng);
        config.king_open_file_penalty = vary_numeric(default_config.king_open_file_penalty, &mut rng);
        config.king_attackers_weight = vary_numeric(default_config.king_attackers_weight, &mut rng);
        config.threat_analysis_weight = vary_numeric(default_config.threat_analysis_weight, &mut rng);
        config.tempo_bonus_weight = vary_numeric(default_config.tempo_bonus_weight, &mut rng);
        config.space_evaluation_weight = vary_numeric(default_config.space_evaluation_weight, &mut rng);
        config.initiative_evaluation_weight = vary_numeric(default_config.initiative_evaluation_weight, &mut rng);

        let file_path = generation_dir.join(format!("individual_{i}.json"));
        let json = serde_json::to_string_pretty(&config).expect("Failed to serialize config");
        fs::write(file_path, json).expect("Failed to write config file");
    }
}

fn vary_numeric(value: i32, rng: &mut impl Rng) -> i32 {
    let factor = rng.gen_range(-0.5..=0.5);
    (value as f64 * (1.0 + factor)).round() as i32
}
