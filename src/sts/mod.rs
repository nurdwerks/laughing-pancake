// src/sts/mod.rs

use crate::event::{Event, StsUpdate, EVENT_BROKER};
use crate::game::search::evaluation_cache::EvaluationCache;
use crate::game::search::{mcts::MctsSearcher, PvsSearcher, SearchAlgorithm, SearchConfig, Searcher};
use shakmaty::{san::San, Chess};
use serde::{Deserialize, Serialize};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::io;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::{fs};


#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct StsResult {
    pub config_hash: u64,
    pub config: SearchConfig,
    pub completed_positions: usize,
    pub correct_moves: usize,
    pub total_positions: usize,
    pub elo: Option<f64>,
}

pub struct StsRunner {
    config: SearchConfig,
    config_hash: u64,
    result: StsResult,
}

impl StsRunner {
    pub fn new(config: SearchConfig) -> Self {
        let mut hasher = DefaultHasher::new();
        config.hash(&mut hasher);
        let config_hash = hasher.finish();

        let result = StsResult {
            config_hash,
            config: config.clone(),
            completed_positions: 0,
            correct_moves: 0,
            total_positions: 0,
            elo: None,
        };

        Self {
            config,
            config_hash,
            result,
        }
    }

    pub fn config_hash(&self) -> u64 {
        self.config_hash
    }

    pub async fn run(&mut self) -> Option<StsResult> {
        let sts_dir = Path::new("sts");
        let results_dir = Path::new("sts_results");
        if !results_dir.exists() {
            fs::create_dir_all(results_dir).expect("Failed to create STS results directory");
        }

        let result_path = results_dir.join(format!("{}.json", self.config_hash));
        if result_path.exists() {
            if let Ok(json) = fs::read_to_string(&result_path) {
                if let Ok(result) = serde_json::from_str::<StsResult>(&json) {
                    self.result = result;
                }
            }
        }

        let epd_files = match get_epd_files(sts_dir) {
            Ok(files) => files,
            Err(e) => {
                eprintln!("Error getting EPD files: {e}");
                return None;
            }
        };

        self.result.total_positions = epd_files.iter().map(|f| parse_epd(f).map(|p| p.len()).unwrap_or(0)).sum();

        let mut pvs_searcher =
            PvsSearcher::with_shared_cache(Arc::new(Mutex::new(EvaluationCache::new())));
        let mut mcts_searcher = MctsSearcher::new();

        let mut current_position_index = 0;

        for file in epd_files {
            let positions = match parse_epd(&file) {
                Ok(positions) => positions,
                Err(e) => {
                    eprintln!("Error parsing EPD file {file:?}: {e}");
                    continue;
                }
            };

            for (pos, best_move_san) in positions {
                if current_position_index < self.result.completed_positions {
                    current_position_index += 1;
                    continue; // Skip already completed positions
                }

                let fen = shakmaty::fen::Fen::from_position(&pos, shakmaty::EnPassantMode::Legal);
                let (best_move, _, _, _) = match self.config.search_algorithm {
                    SearchAlgorithm::Pvs => pvs_searcher.search(
                        &pos,
                        self.config.search_depth,
                        &self.config,
                        false,
                        false,
                    ),
                    SearchAlgorithm::Mcts => mcts_searcher.search(
                        &pos,
                        self.config.search_depth,
                        &self.config,
                        false,
                        false,
                    ),
                };

                let (is_correct, move_san) = if let Some(m) = best_move {
                    let san = San::from_move(&pos, m);
                    let san_str = san.to_string();
                    (san_str == best_move_san, san_str)
                } else {
                    (false, "None".to_string())
                };

                if is_correct {
                    self.result.correct_moves += 1;
                }

                println!(
                    "[STS] [{}/{}] {} -> {} ({}) [{}]",
                    self.result.completed_positions + 1,
                    self.result.total_positions,
                    fen,
                    move_san,
                    best_move_san,
                    if is_correct { "Match" } else { "Fail" }
                );
                self.result.completed_positions += 1;
                current_position_index += 1;

            let progress =
                self.result.completed_positions as f64 / self.result.total_positions as f64;
            EVENT_BROKER.publish(Event::StsUpdate(StsUpdate {
                config_hash: self.result.config_hash,
                progress,
                score: self.result.correct_moves,
                total: self.result.total_positions,
                elo: None,
            }));

            // Save progress every 10 positions
            if self.result.completed_positions % 10 == 0 {
                let json = serde_json::to_string_pretty(&self.result).unwrap();
                fs::write(&result_path, &json).expect("Failed to save STS result");
            }
            }
        }

        // Finalize and save the result
        let score_percentage =
            (self.result.correct_moves as f64 / self.result.total_positions as f64) * 100.0;
        self.result.elo = Some(44.523 * score_percentage - 242.85);

        EVENT_BROKER.publish(Event::StsUpdate(StsUpdate {
            config_hash: self.result.config_hash,
            progress: 1.0,
            score: self.result.correct_moves,
            total: self.result.total_positions,
            elo: self.result.elo,
        }));

        let json = serde_json::to_string_pretty(&self.result).unwrap();
        fs::write(result_path, json).expect("Failed to save final STS result");

        Some(self.result.clone())
    }
}


fn get_epd_files(dir: &Path) -> io::Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    if dir.is_dir() {
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_file() && path.extension().and_then(|s| s.to_str()) == Some("epd") {
                files.push(path);
            }
        }
    }
    Ok(files)
}

fn parse_epd(file_path: &Path) -> Result<Vec<(Chess, String)>, String> {
    let content = fs::read_to_string(file_path)
        .map_err(|e| format!("Failed to read EPD file: {e}"))?;

    let mut positions = Vec::new();
    for line in content.lines() {
        if line.trim().is_empty() {
            continue;
        }

        let parts: Vec<&str> = line.split(" bm ").collect();
        if parts.len() != 2 {
            continue;
        }

        let fen_str = parts[0];
        let best_move_str = parts[1].split(';').next().unwrap_or("").trim();

        let fen: shakmaty::fen::Fen = fen_str.parse().map_err(|e| format!("Failed to parse EPD line: {e}"))?;
        let pos: Chess = fen.into_position(shakmaty::CastlingMode::Standard).map_err(|e| format!("Failed to setup position: {e}"))?;
        positions.push((pos, best_move_str.to_string()));
    }

    Ok(positions)
}
