use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use rand::Rng;
use rand::distributions::Distribution;
use rand::seq::SliceRandom;
use shakmaty::{Chess, Position, zobrist::{Zobrist64, ZobristHash}, EnPassantMode};
use shakmaty::san::SanPlus;
use serde::{Deserialize, Serialize};

use crate::app::Worker;
use crate::constants::{NUM_ROUNDS, STARTING_ELO, POPULATION_SIZE, MUTATION_CHANCE, ENABLE_MOVE_LIMIT};
use crate::game::search::{self, SearchConfig, SearchAlgorithm, PvsSearcher, Searcher, evaluation_cache::EvaluationCache};
use crate::event::{Event, MatchResult, EVENT_BROKER};

const EVOLUTION_DIR: &str = "evolution";

/// Manages evaluation caches for all players in the tournament.
/// Caches are created on-demand and automatically destroyed when no longer in use.
struct CacheManager {
    caches: Arc<Mutex<HashMap<SearchConfig, Arc<Mutex<EvaluationCache>>>>>,
    usage_count: Arc<Mutex<HashMap<SearchConfig, usize>>>,
}

impl CacheManager {
    fn new() -> Self {
        Self {
            caches: Arc::new(Mutex::new(HashMap::new())),
            usage_count: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Gets a cache for a specific AI configuration.
    /// This will create a cache if one doesn't exist.
    /// It returns a `CacheGuard`, which will automatically decrement the usage count
    /// when it goes out of scope.
    fn get_cache_for_config(&self, config: &SearchConfig) -> CacheGuard {
        let mut caches = self.caches.lock().unwrap();
        let mut usage_count = self.usage_count.lock().unwrap();

        let cache = caches
            .entry(config.clone())
            .or_insert_with(|| Arc::new(Mutex::new(EvaluationCache::new())))
            .clone();

        *usage_count.entry(config.clone()).or_insert(0) += 1;

        CacheGuard {
            config: config.clone(),
            cache: cache.clone(),
            cache_manager: self.clone(),
        }
    }

    /// Called by `CacheGuard` when it's dropped.
    /// Decrements the usage count for a player's cache and removes it if the count is zero.
    fn release_cache(&self, config: &SearchConfig) {
        let mut caches = self.caches.lock().unwrap();
        let mut usage_count = self.usage_count.lock().unwrap();

        if let Some(count) = usage_count.get_mut(config) {
            *count -= 1;
            if *count == 0 {
                usage_count.remove(config);
                caches.remove(config);
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
    config: SearchConfig,
    cache: Arc<Mutex<EvaluationCache>>,
    cache_manager: CacheManager,
}

impl Drop for CacheGuard {
    fn drop(&mut self) {
        self.cache_manager.release_cache(&self.config);
    }
}


/// Manages the evolution process in a background thread.
#[derive(Clone)]
pub struct EvolutionManager {
    workers: Arc<Mutex<Vec<Worker>>>,
    should_quit: Arc<Mutex<bool>>,
    match_id_counter: Arc<Mutex<usize>>,
}

impl EvolutionManager {
    pub fn new(
        workers: Arc<Mutex<Vec<Worker>>>,
        should_quit: Arc<Mutex<bool>>,
        match_id_counter: Arc<Mutex<usize>>,
    ) -> Self {
        Self {
            workers,
            should_quit,
            match_id_counter,
        }
    }

    fn send_status(&self, message: String) -> Result<(), ()> {
        EVENT_BROKER.publish(Event::StatusUpdate(message));
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
            EVENT_BROKER.publish(Event::Panic(panic_msg));
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


        let cache_manager = CacheManager::new();

        loop {
            if *self.should_quit.lock().unwrap() {
                self.send_status("Shutdown signal received, stopping evolution.".to_string())?;
                break Ok(());
            }
            self.send_status(format!("--- Starting Generation {generation_index} ---"))?;
            EVENT_BROKER.publish(Event::GenerationStarted(generation_index));
            let generation_dir = setup_directories(generation_index);

            let base_population = Population::load(&generation_dir);
            let mut generation = self.load_or_create_generation(generation_index, &base_population)?;
            self.send_status(format!("Loaded {} individuals for generation {generation_index}.", generation.population.individuals.len()))?;

            self.run_tournament(&mut generation, &cache_manager)?;

            let next_generation_dir = setup_directories(generation_index + 1);
            self.evolve_population(&generation, &next_generation_dir)?;
            self.send_status(format!("--- Generation {generation_index} Complete ---"))?;
            generation_index += 1;
        }
    }

    /// Loads a generation from a file, or creates a new one if it doesn't exist or is corrupt.
    fn load_or_create_generation(&self, generation_index: u32, population: &Population) -> Result<Generation, ()> {
        let file_path = Path::new(EVOLUTION_DIR)
            .join(format!("generation_{generation_index}.json"));

        if file_path.exists() {
            if let Ok(json_content) = fs::read_to_string(&file_path) {
                if let Ok(mut gen) = serde_json::from_str::<Generation>(&json_content) {
                    self.send_status(format!("Successfully loaded existing match data for generation {generation_index}."))?;
                    // Ensure the population loaded from the JSON is used, as it contains ELO scores.
                    // The population passed in is the base from the individual files, without ELO updates.
                    gen.population.individuals.sort_by_key(|i| i.id); // Ensure consistent order
                    return Ok(gen);
                } else {
                    self.send_status(format!("Warning: Found corrupt generation file at {file_path:?}. Starting generation from scratch."))?;
                }
            } else {
                 self.send_status(format!("Warning: Could not read generation file at {file_path:?}. Starting generation from scratch."))?;
            }
        }

        self.send_status(format!("No existing match data found for generation {generation_index}. Creating new tournament."))?;

        let mut shuffled_population = population.clone();
        shuffled_population
            .individuals
            .shuffle(&mut rand::thread_rng());

        Ok(Generation {
            generation_index,
            round: 1,
            population: shuffled_population,
            matches: Vec::new(),
            previous_matchups: HashSet::new(),
            white_games_played: HashMap::new(),
            black_games_played: HashMap::new(),
            round_pairings: Vec::new(),
        })
    }

    /// Takes a completed tournament population and evolves it to create the next generation.
    fn evolve_population(&self, generation: &Generation, next_generation_dir: &Path) -> Result<(), ()> {
        self.send_status("\nEvolving to the next generation...".to_string())?;
        let mut rng = rand::thread_rng();

        // --- Stage 1: Determine survivors and create the initial pool for the next generation ---
        let mut win_counts = HashMap::new();
        for m in &generation.matches {
            if m.result == "1-0" {
                let winner_id = parse_id_from_name(&m.white_player_name);
                *win_counts.entry(winner_id).or_insert(0) += 1;
            } else if m.result == "0-1" {
                let winner_id = parse_id_from_name(&m.black_player_name);
                *win_counts.entry(winner_id).or_insert(0) += 1;
            }
        }

        let winners: Vec<Individual> = generation.population.individuals.iter()
            .filter(|i| win_counts.get(&i.id).cloned().unwrap_or(0) >= 1)
            .cloned()
            .collect();

        let mut next_generation_pool: Vec<Individual> = Vec::with_capacity(POPULATION_SIZE * 2);

        if !winners.is_empty() {
            // --- Winner Scenario ---
            self.send_status(format!("{} individuals secured at least one win and will form the elite breeding pool.", winners.len()))?;

            // Survivors are the winners. Add them to the pool.
            next_generation_pool.extend(winners.clone());

            let num_offspring = (POPULATION_SIZE as f64 / 2.0).ceil() as usize;
            let num_random = POPULATION_SIZE.saturating_sub(num_offspring);

            self.send_status(format!("Breeding {} new offspring.", num_offspring))?;
            for _ in 0..num_offspring {
                let parent1 = winners.choose(&mut rng).unwrap();
                let parent2 = winners.choose(&mut rng).unwrap();
                let mut child_config = crossover(&parent1.config, &parent2.config, &mut rng);
                mutate(&mut child_config, &mut rng);
                next_generation_pool.push(Individual {
                    id: 0, // Placeholder
                    config: child_config,
                    elo: STARTING_ELO,
                });
            }

            self.send_status(format!("Introducing {} new random individuals.", num_random))?;
            for _ in 0..num_random {
                next_generation_pool.push(Individual {
                    id: 0, // Placeholder
                    config: SearchConfig::default_with_randomization(&mut rng),
                    elo: STARTING_ELO,
                });
            }

        } else {
            // --- No-Winner Scenario ---
            self.send_status("No individual secured a win. Replacing the bottom 25% by ELO.".to_string())?;
            let mut sorted_population = generation.population.individuals.clone();
            sorted_population.sort_by(|a, b| b.elo.partial_cmp(&a.elo).unwrap_or(std::cmp::Ordering::Equal));

            let num_survivors = (POPULATION_SIZE as f64 * 0.75).round() as usize;
            let num_to_replace = POPULATION_SIZE - num_survivors;

            let survivors: Vec<Individual> = sorted_population.into_iter().take(num_survivors).collect();
            next_generation_pool.extend(survivors);

            self.send_status(format!("Replacing {} individuals with new random ones.", num_to_replace))?;
            for _ in 0..num_to_replace {
                 next_generation_pool.push(Individual {
                    id: 0, // Placeholder
                    config: SearchConfig::default_with_randomization(&mut rng),
                    elo: STARTING_ELO,
                });
            }
        }

        // --- Stage 2: Filter out clones, keeping the one with the highest ELO ---
        let initial_pool_size = next_generation_pool.len();
        let mut unique_individuals = HashMap::new();
        for individual in next_generation_pool {
            unique_individuals.entry(individual.config.clone())
                .and_modify(|existing: &mut Individual| {
                    if individual.elo > existing.elo {
                        *existing = individual.clone();
                    }
                })
                .or_insert(individual);
        }

        let mut next_generation: Vec<Individual> = unique_individuals.values().cloned().collect();
        let num_clones_removed = initial_pool_size - next_generation.len();
        self.send_status(format!("Removed {} clone(s) from the population.", num_clones_removed))?;

        // --- Stage 3: Repopulate if necessary, ensuring new individuals are not clones ---
        while next_generation.len() < POPULATION_SIZE {
            let new_config = SearchConfig::default_with_randomization(&mut rng);

            // Ensure the new random config is not already in the unique set
            if !unique_individuals.contains_key(&new_config) {
                let new_individual = Individual {
                    id: 0, // Placeholder
                    config: new_config.clone(),
                    elo: STARTING_ELO,
                };
                unique_individuals.insert(new_config, new_individual.clone());
                next_generation.push(new_individual);
            }
        }

        // If we have too many, truncate the list. This could happen if the initial pool had
        // more than POPULATION_SIZE unique individuals. We'll sort by ELO and take the top.
        if next_generation.len() > POPULATION_SIZE {
            next_generation.sort_by(|a, b| b.elo.partial_cmp(&a.elo).unwrap_or(std::cmp::Ordering::Equal));
            next_generation.truncate(POPULATION_SIZE);
            self.send_status(format!("Population truncated to {} individuals based on ELO.", POPULATION_SIZE))?;
        }

        // --- Stage 4: Finalize and save the new generation ---
        for (i, individual) in next_generation.iter_mut().enumerate() {
            individual.id = i;
            let individual_path = next_generation_dir.join(format!("individual_{}.json", i));
            let json = serde_json::to_string_pretty(individual).expect("Failed to serialize individual");
            fs::write(individual_path, json).expect("Failed to write individual file");
        }

        self.send_status(format!("Generated and saved {} unique individuals for the next generation.", next_generation.len()))?;
        Ok(())
    }

    /// Runs a 7-round Swiss tournament using the Dutch pairing system.
    fn run_tournament(
        &self,
        generation: &mut Generation,
        cache_manager: &CacheManager,
    ) -> Result<(), ()> {
        self.send_status(format!(
            "Running tournament for generation {}",
            generation.generation_index
        ))?;

        for round in generation.round..=NUM_ROUNDS {
            if *self.should_quit.lock().unwrap() {
                self.send_status("Shutdown signal received, stopping tournament.".to_string())?;
                break;
            }

            generation.round = round;
            self.send_status(format!("\n--- Round {round}/{NUM_ROUNDS} ---"))?;

            if generation.round_pairings.is_empty() {
                self.send_status("Generating pairings for the round.".to_string())?;
                generation.round_pairings =
                    self.generate_pairings(generation, round);
                save_generation(generation);
            } else {
                self.send_status("Using existing pairings for the round.".to_string())?;
            }

            // 2. Play the matches for the current round
            let round_matches = generation.round_pairings.clone();
            self.play_round_matches(&round_matches, generation, cache_manager)?;
            generation.round_pairings.clear();

            // 3. Save state after each round
            save_generation(generation);
            self.send_status(format!("Round {round} complete. Saved progress."))?;
        }

    // At the end of the tournament, print final ELOs.
    generation.population.individuals.sort_by(|a, b| b.elo.partial_cmp(&a.elo).unwrap_or(std::cmp::Ordering::Equal));
    self.send_status("\n--- Final Tournament Standings ---".to_string())?;
    for (rank, individual) in generation.population.individuals.iter().enumerate() {
        self.send_status(format!(
            "#{:<3} Individual {:<3} | ELO: {:.2}",
            rank + 1,
            individual.id,
            individual.elo
        ))?;
    }
    Ok(())
}

fn generate_pairings(&self, generation: &mut Generation, round: u32) -> Vec<Match> {
    let mut round_matches = Vec::new();
    let mut paired_ids = HashSet::new();

    // Group players by ELO score.
    let mut score_groups: HashMap<i32, Vec<Individual>> = HashMap::new();
    for individual in &generation.population.individuals {
        // Use ELO rounded to nearest 100 for broader groups
        let elo_group = (individual.elo / 100.0).round() as i32 * 100;
        score_groups
            .entry(elo_group)
            .or_default()
            .push(individual.clone());
    }

    // Sort score groups by ELO
    let mut sorted_score_groups = score_groups.into_iter().collect::<Vec<_>>();
    sorted_score_groups.sort_by_key(|(elo, _)| -*elo);

    let mut unpaired_players = Vec::new();

    for (_, mut group) in sorted_score_groups {
        group.sort_by(|a, b| {
            b.elo
                .partial_cmp(&a.elo)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        group.append(&mut unpaired_players); // Add players from previous smaller groups

        while group.len() >= 2 {
            let p1 = group.remove(0);
            let mut opponent_found = false;

            for i in 0..group.len() {
                let p2 = &group[i];
                let matchup = (p1.id.min(p2.id), p1.id.max(p2.id));

                if !generation.previous_matchups.contains(&matchup) {
                    let p2 = group.remove(i);
                    let (white, black) = self.assign_colors(
                        &p1,
                        &p2,
                        &generation.white_games_played,
                        &generation.black_games_played,
                    );

                    let white_player_name = format!("individual_{}.json", white.id);
                    let black_player_name = format!("individual_{}.json", black.id);

                    // Check if this match has already been played in this round
                    let match_played = generation.matches.iter().any(|m| {
                        m.round == round
                            && m.white_player_name == white_player_name
                            && m.black_player_name == black_player_name
                    });

                    if !match_played {
                        round_matches.push(Match {
                            round,
                            white_player_name,
                            black_player_name,
                            status: "pending".to_string(),
                            result: "".to_string(),
                            san: "".to_string(),
                        });
                    }

                    generation.previous_matchups.insert(matchup);
                    paired_ids.insert(p1.id);
                    paired_ids.insert(p2.id);
                    *generation.white_games_played.entry(white.id).or_insert(0) += 1;
                    *generation.black_games_played.entry(black.id).or_insert(0) += 1;

                    opponent_found = true;
                    break;
                }
            }

            if !opponent_found {
                unpaired_players.push(p1);
            }
        }
        unpaired_players.extend(group); // Add remaining players
    }
    round_matches
}


/// Plays a set of matches, updating population and generation state.
fn play_round_matches(
    &self,
    matches_to_play: &[Match],
    generation: &mut Generation,
    cache_manager: &CacheManager,
) -> Result<(), ()> {
    // Identify completed matches for the current round to avoid re-playing them.
    let completed_match_keys: HashSet<(String, String)> = generation
        .matches
        .iter()
        .filter(|m| m.round == generation.round)
        .map(|m| (m.white_player_name.clone(), m.black_player_name.clone()))
        .collect();

    let pending_matches: Vec<Match> = matches_to_play
        .iter()
        .filter(|m| {
            !completed_match_keys.contains(&(m.white_player_name.clone(), m.black_player_name.clone()))
        })
        .cloned()
        .collect();

    let skipped_matches = matches_to_play.len() - pending_matches.len();
    if skipped_matches > 0 {
        self.send_status(format!(
            "Skipping {} completed matches for round {}.",
            skipped_matches, generation.round
        ))?;
    }

    let total_matches = matches_to_play.len();
    EVENT_BROKER.publish(Event::TournamentStart(
        generation.round as usize,
        total_matches,
        skipped_matches,
    ));

    let (results_tx, results_rx) = crossbeam_channel::unbounded();
    let (jobs_tx, jobs_rx) = crossbeam_channel::unbounded::<(usize, Match)>();
    let population_arc = Arc::new(generation.population.clone());

        // Spawn a pool of worker threads
        const NUM_WORKERS: usize = 3;
        let mut worker_handles = Vec::new();
        for _ in 0..NUM_WORKERS {
            let jobs_rx_clone = jobs_rx.clone();
            let results_tx_clone = results_tx.clone();
            let population_clone = Arc::clone(&population_arc);
            let cache_manager_clone = cache_manager.clone();
            let self_clone = self.clone();

            let handle = std::thread::spawn(move || {
                while let Ok((match_index, game_match)) = jobs_rx_clone.recv() {
                    let white_player_name = game_match.white_player_name.clone();
                    let black_player_name = game_match.black_player_name.clone();

                    let white_id = parse_id_from_name(&white_player_name);
                    let black_id = parse_id_from_name(&black_player_name);
                    let white_config = &population_clone.individuals[white_id].config;
                    let black_config = &population_clone.individuals[black_id].config;

                    let white_cache_guard = cache_manager_clone.get_cache_for_config(white_config);
                    let black_cache_guard = cache_manager_clone.get_cache_for_config(black_config);

                    EVENT_BROKER.publish(Event::MatchStarted(match_index, white_player_name, black_player_name));
                    if let Ok((result, san)) = self_clone.play_game(match_index, white_config, black_config, &white_cache_guard, &black_cache_guard) {
                        results_tx_clone.send((match_index, game_match, result, san)).unwrap_or(());
                    }
                }
            });
            worker_handles.push(handle);
        }

        // Send all jobs to the workers
    for game_match in pending_matches.iter().cloned() {
            if *self.should_quit.lock().unwrap() {
                break;
            }
            let mut counter = self.match_id_counter.lock().unwrap();
            let match_id = *counter;
            *counter += 1;
            if jobs_tx.send((match_id, game_match)).is_err() {
                // This would happen if all worker threads panicked and the channel is closed.
                break;
            }
        }
    drop(jobs_tx);

    drop(results_tx); // Drop original sender

        // Process results as they come in
    for (match_index, mut current_match, result, san) in results_rx {
            current_match.san = san;
            current_match.status = "completed".to_string();

            let white_id = parse_id_from_name(&current_match.white_player_name);
            let black_id = parse_id_from_name(&current_match.black_player_name);

        let new_white_elo;
        let new_black_elo;
        {
            let population = &mut generation.population;
            let white_elo = population.individuals.iter().find(|i| i.id == white_id).unwrap().elo;
            let black_elo = population.individuals.iter().find(|i| i.id == black_id).unwrap().elo;

            (new_white_elo, new_black_elo) = match result {
                GameResult::WhiteWin => {
                    current_match.result = "1-0".to_string();
                    update_elo(white_elo, black_elo, 1.0)
                }
                GameResult::BlackWin => {
                    current_match.result = "0-1".to_string();
                    update_elo(white_elo, black_elo, 0.0)
                }
                GameResult::Draw => {
                    current_match.result = "1/2-1/2".to_string();
                    update_elo(white_elo, black_elo, 0.5)
                }
            };
            population.individuals.iter_mut().find(|i| i.id == white_id).unwrap().elo = new_white_elo;
            population.individuals.iter_mut().find(|i| i.id == black_id).unwrap().elo = new_black_elo;
        }

        generation.matches.push(current_match.clone());
            save_generation(generation);

            let result_event = MatchResult {
                white_player_name: current_match.white_player_name.clone(),
                black_player_name: current_match.black_player_name.clone(),
                white_new_elo: new_white_elo,
                black_new_elo: new_black_elo,
                result: current_match.result.clone(),
            };
            EVENT_BROKER.publish(Event::MatchCompleted(match_index, result_event));
        }

        // Wait for all worker threads to finish
        for handle in worker_handles {
            handle.join().unwrap();
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
        let mut position_counts: HashMap<u64, u32> = HashMap::new();
        let mut game_result_override = None;
        while !pos.is_game_over() {
            // End the game in a draw after 100 moves (200 half-moves/plies).
            if ENABLE_MOVE_LIMIT && sans.len() >= 200 {
                game_result_override = Some(GameResult::Draw);
                break;
            }
            let zobrist_hash: Zobrist64 = pos.zobrist_hash(EnPassantMode::Legal);
            let count = position_counts.entry(zobrist_hash.0).or_insert(0);
            *count += 1;
            if *count >= 4 {
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
            EVENT_BROKER.publish(Event::ThinkingUpdate(match_id, thinking_msg, 0));

            let workers = self.workers.clone();
            crossbeam_utils::thread::scope(|s| {
                s.spawn(|_| {
                    let search_result = searcher.search(&current_pos, config.search_depth, &config, Some(workers), Some(match_id));
                    search_result_tx.send(search_result).unwrap();
                });

                crossbeam_channel::select! {
                    recv(search_result_rx) -> msg => {
                        if let Ok((best_move, eval, _final_tree)) = msg {
                            EVENT_BROKER.publish(Event::ThinkingUpdate(match_id, format!("AI finished thinking for {:?}...", current_pos.turn()), eval));
                            if let Some(m) = best_move {
                                let san = SanPlus::from_move(pos.clone(), m);
                                sans.push(san);
                                pos.play_unchecked(m);

                                let material_diff = calculate_material_difference(&pos);
                                let last_san = sans.last().map(|s| s.to_string()).unwrap_or_default();
                                EVENT_BROKER.publish(Event::MovePlayed(match_id, last_san, material_diff, pos.clone()));
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

    fn assign_colors<'a>(
        &self,
        p1: &'a Individual,
        p2: &'a Individual,
        white_games_played: &HashMap<usize, u32>,
        black_games_played: &HashMap<usize, u32>,
    ) -> (&'a Individual, &'a Individual) {
        let p1_white_games = white_games_played.get(&p1.id).cloned().unwrap_or(0);
        let p1_black_games = black_games_played.get(&p1.id).cloned().unwrap_or(0);
        let p2_white_games = white_games_played.get(&p2.id).cloned().unwrap_or(0);
        let p2_black_games = black_games_played.get(&p2.id).cloned().unwrap_or(0);

        // Player with fewer white games plays white
        if p1_white_games < p2_white_games {
            return (p1, p2);
        }
        if p2_white_games < p1_white_games {
            return (p2, p1);
        }

        // Player with more black games plays white
        if p1_black_games > p2_black_games {
            return (p1, p2);
        }
        if p2_black_games > p1_black_games {
            return (p2, p1);
        }

        // Higher ELO plays white as a tie-breaker
        if p1.elo >= p2.elo {
            (p1, p2)
        } else {
            (p2, p1)
        }
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
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Individual {
    pub id: usize,
    #[serde(flatten)]
    pub config: SearchConfig,
    pub elo: f64,
}

/// Represents a collection of individuals for a single generation.
#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct Population {
    pub individuals: Vec<Individual>,
}

impl Population {
    /// Loads a population from a generation directory.
    pub fn load(generation_dir: &Path) -> Self {
        let mut individuals = Vec::new();
        let mut rng = rand::thread_rng();

        for i in 0..POPULATION_SIZE {
            let file_path = generation_dir.join(format!("individual_{i}.json"));
            let individual = match fs::read_to_string(&file_path) {
                Ok(json) => match serde_json::from_str::<Individual>(&json) {
                    Ok(mut ind) => {
                        if ind.id != i {
                            let warning_msg = format!(
                                "Warning: Individual file {:?} has mismatched ID {}. Correcting to {}.",
                                file_path, ind.id, i
                            );
                            EVENT_BROKER.publish(Event::StatusUpdate(warning_msg));
                            ind.id = i;
                        }
                        ind
                    }
                    Err(e) => {
                        let warning_msg = format!(
                            "Warning: Failed to deserialize {:?}: {}. Replacing with new random individual.",
                            file_path, e
                        );
                        EVENT_BROKER.publish(Event::StatusUpdate(warning_msg));
                        Individual {
                            id: i,
                            config: SearchConfig::default_with_randomization(&mut rng),
                            elo: STARTING_ELO,
                        }
                    }
                },
                Err(e) => {
                    let warning_msg = format!(
                        "Warning: Failed to read {:?}: {}. Replacing with new random individual.",
                        file_path, e
                    );
                    EVENT_BROKER.publish(Event::StatusUpdate(warning_msg));
                    Individual {
                        id: i,
                        config: SearchConfig::default_with_randomization(&mut rng),
                        elo: STARTING_ELO,
                    }
                }
            };
            individuals.push(individual);
        }
        Self { individuals }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Match {
    pub round: u32,
    pub white_player_name: String,
    pub black_player_name: String,
    pub status: String, // "pending", "completed"
    pub result: String, // "1-0", "0-1", "1/2-1/2", ""
    pub san: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Generation {
    pub generation_index: u32,
    pub round: u32,
    pub population: Population,
    pub matches: Vec<Match>,
    #[serde(with = "serde_helpers::hash_set_tuple_vec")]
    pub previous_matchups: HashSet<(usize, usize)>,
    #[serde(default)]
    pub white_games_played: HashMap<usize, u32>,
    #[serde(default)]
    pub black_games_played: HashMap<usize, u32>,
    #[serde(default)]
    pub round_pairings: Vec<Match>,
}

// Helper for serializing HashSet<(A, B)>
mod serde_helpers {
    use serde::{Deserialize, Deserializer, Serialize, Serializer};
    use std::collections::HashSet;
    use std::hash::Hash;

    pub mod hash_set_tuple_vec {
        use super::*;

        pub fn serialize<A, B, S>(set: &HashSet<(A, B)>, serializer: S) -> Result<S::Ok, S::Error>
        where
            A: Serialize + Eq + Hash,
            B: Serialize + Eq + Hash,
            S: Serializer,
        {
            let vec: Vec<_> = set.iter().collect();
            vec.serialize(serializer)
        }

        pub fn deserialize<'de, A, B, D>(deserializer: D) -> Result<HashSet<(A, B)>, D::Error>
        where
            A: Deserialize<'de> + Eq + Hash,
            B: Deserialize<'de> + Eq + Hash,
            D: Deserializer<'de>,
        {
            let vec: Vec<(A, B)> = Vec::deserialize(deserializer)?;
            Ok(vec.into_iter().collect())
        }
    }
}

/// Saves the current state of a generation to a JSON file.
pub fn save_generation(generation: &Generation) {
    let file_path = Path::new(EVOLUTION_DIR)
        .join(format!("generation_{}.json", generation.generation_index));
    let json = serde_json::to_string_pretty(generation).expect("Failed to serialize generation state");
    fs::write(file_path, json).expect("Failed to write generation state file");
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum GameResult {
    WhiteWin,
    BlackWin,
    Draw,
}

/// Calculates the new ELO ratings for two players based on the game outcome.
///
/// # Arguments
/// * `white_elo` - The ELO rating of the white player.
/// * `black_elo` - The ELO rating of the black player.
/// * `score` - The score of the white player (1.0 for a win, 0.5 for a draw, 0.0 for a loss).
///
/// # Returns
/// A tuple containing the new ELO for white and black, respectively.
fn update_elo(white_elo: f64, black_elo: f64, score: f64) -> (f64, f64) {
    const K_FACTOR: f64 = 32.0;

    let expected_score_white = 1.0 / (1.0 + 10.0f64.powf((black_elo - white_elo) / 400.0));
    let expected_score_black = 1.0 - expected_score_white;

    let new_white_elo = white_elo + K_FACTOR * (score - expected_score_white);
    let new_black_elo = black_elo + K_FACTOR * ((1.0 - score) - expected_score_black);

    (new_white_elo, new_black_elo)
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
        enhanced_king_attack_weight: if rng.gen_bool(0.5) { p1.enhanced_king_attack_weight } else { p2.enhanced_king_attack_weight },
        advanced_passed_pawn_weight: if rng.gen_bool(0.5) { p1.advanced_passed_pawn_weight } else { p2.advanced_passed_pawn_weight },
        opponent_weakness_weight: if rng.gen_bool(0.5) { p1.opponent_weakness_weight } else { p2.opponent_weakness_weight },
        contempt_factor: if rng.gen_bool(0.5) { p1.contempt_factor } else { p2.contempt_factor },
        draw_avoidance_margin: if rng.gen_bool(0.5) { p1.draw_avoidance_margin } else { p2.draw_avoidance_margin },
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
    config.enhanced_king_attack_weight = mutate_numeric(config.enhanced_king_attack_weight, rng);
    config.advanced_passed_pawn_weight = mutate_numeric(config.advanced_passed_pawn_weight, rng);
    config.opponent_weakness_weight = mutate_numeric(config.opponent_weakness_weight, rng);
    config.contempt_factor = mutate_numeric(config.contempt_factor, rng);
    config.draw_avoidance_margin = mutate_numeric(config.draw_avoidance_margin, rng);
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
        let individual = Individual {
            id: i,
            config: SearchConfig::default_with_randomization(&mut rng),
            elo: STARTING_ELO,
        };
        let file_path = generation_dir.join(format!("individual_{i}.json"));
        let json = serde_json::to_string_pretty(&individual).expect("Failed to serialize individual");
        fs::write(file_path, json).expect("Failed to write individual file");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn create_mock_individual(id: usize, elo: f64) -> Individual {
        Individual {
            id,
            config: SearchConfig::default(),
            elo,
        }
    }

    #[test]
    fn test_evolve_population_winner_scenario() {
        let temp_dir = tempdir().unwrap();
        let next_gen_dir = temp_dir.path().join("generation_1");
        fs::create_dir(&next_gen_dir).unwrap();

        let mut individuals = Vec::new();
        // Create a population where one individual clearly won
        individuals.push(create_mock_individual(0, 1300.0)); // Winner
        individuals.push(create_mock_individual(1, 1100.0));
        for i in 2..POPULATION_SIZE {
            individuals.push(create_mock_individual(i, 1200.0));
        }

        let population = Population { individuals };
        let mut generation = Generation {
            generation_index: 0,
            round: NUM_ROUNDS,
            population,
            matches: vec![
                Match { round: 1, white_player_name: "individual_0.json".to_string(), black_player_name: "individual_1.json".to_string(), status: "completed".to_string(), result: "1-0".to_string(), san: "".to_string() }
            ],
            previous_matchups: HashSet::new(),
            white_games_played: HashMap::new(),
            black_games_played: HashMap::new(),
            round_pairings: Vec::new(),
        };

        let evolution_manager = EvolutionManager::new(Arc::new(Mutex::new(vec![])), Arc::new(Mutex::new(false)), Arc::new(Mutex::new(0)));
        evolution_manager.evolve_population(&mut generation, &next_gen_dir).unwrap();

        // Check that the next generation was created
        assert!(next_gen_dir.join("individual_0.json").exists());
        let next_gen_population = Population::load(&next_gen_dir);
        assert_eq!(next_gen_population.individuals.len(), POPULATION_SIZE);

        // Verify that the winner is among the survivors in the new population's gene pool (indirectly)
        // This is tricky to assert directly, but we can check the number of files.
        let num_files = fs::read_dir(next_gen_dir).unwrap().count();
        assert_eq!(num_files, POPULATION_SIZE);
    }

    #[test]
    fn test_evolve_population_no_winner_scenario() {
        let temp_dir = tempdir().unwrap();
        let next_gen_dir = temp_dir.path().join("generation_1");
        fs::create_dir(&next_gen_dir).unwrap();

        let mut individuals = Vec::new();
        // Create a population where everyone draws, but with different ELOs
        for i in 0..POPULATION_SIZE {
            individuals.push(create_mock_individual(i, 1200.0 + (i as f64 * 10.0)));
        }

        let population = Population { individuals };
        let mut generation = Generation {
            generation_index: 0,
            round: NUM_ROUNDS,
            population,
            matches: vec![
                 Match { round: 1, white_player_name: "individual_0.json".to_string(), black_player_name: "individual_1.json".to_string(), status: "completed".to_string(), result: "1/2-1/2".to_string(), san: "".to_string() }
            ],
            previous_matchups: HashSet::new(),
            white_games_played: HashMap::new(),
            black_games_played: HashMap::new(),
            round_pairings: Vec::new(),
        };

        let evolution_manager = EvolutionManager::new(Arc::new(Mutex::new(vec![])), Arc::new(Mutex::new(false)), Arc::new(Mutex::new(0)));
        evolution_manager.evolve_population(&mut generation, &next_gen_dir).unwrap();

        // Check that the next generation was created
        let next_gen_population = Population::load(&next_gen_dir);
        assert_eq!(next_gen_population.individuals.len(), POPULATION_SIZE);
    }
}
