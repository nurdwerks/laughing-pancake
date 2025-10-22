// src/sts/mod.rs

use crate::event::{Event, StsUpdate, EVENT_BROKER};
use crate::game::search::SearchConfig;
use crate::worker::{push_job, Job};
use lazy_static::lazy_static;
use shakmaty::Chess;
use std::collections::{hash_map::DefaultHasher, HashSet};
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::{fs, io};
use serde::{Deserialize, Serialize};
use tokio::task;

lazy_static! {
    static ref RUNNING_STS_TESTS: Mutex<HashSet<u64>> = Mutex::new(HashSet::new());
}

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
        {
            let mut running_tests = RUNNING_STS_TESTS.lock().unwrap();
            if running_tests.contains(&self.config_hash) {
                println!(
                    "STS run for config hash {} is already in progress.",
                    self.config_hash
                );
                return None;
            }
            running_tests.insert(self.config_hash);
        }

        let sts_dir = Path::new("sts");
        let results_dir = Path::new("sts_results");
        if !results_dir.exists() {
            fs::create_dir_all(results_dir).expect("Failed to create STS results directory");
        }

        let result_path = results_dir.join(format!("{}.json", self.config_hash));
        if result_path.exists() {
            if let Ok(json) = fs::read_to_string(&result_path) {
                if let Ok(result) = serde_json::from_str::<StsResult>(&json) {
                    println!("Resuming STS run for config hash: {}", self.config_hash);
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

        let mut all_positions = Vec::new();
        for file in epd_files {
            match parse_epd(&file) {
                Ok(positions) => all_positions.extend(positions),
                Err(e) => eprintln!("Error parsing EPD file {file:?}: {e}"),
            }
        }
        self.result.total_positions = all_positions.len();

        let config = self.config.clone();
        let mut result = self.result.clone();
        let config_hash = self.config_hash;

        // This task will manage the entire STS run, distributing jobs to the worker pool.
        let handle = task::spawn(async move {
            let (result_tx, result_rx) = crossbeam_channel::unbounded();

            // Enqueue a job for each position that hasn't been completed yet.
            for (i, (pos, best_move_san)) in all_positions.into_iter().enumerate() {
                if i < result.completed_positions {
                    continue; // Skip already completed positions
                }
                let job = Job::EvaluateStsPosition {
                    pos,
                    best_move_san,
                    config: config.clone(),
                    result_tx: result_tx.clone(),
                };
                push_job(job);
            }
            // Drop the original sender so the receiver knows when all jobs are done.
            drop(result_tx);

            // This loop collects results from the worker pool.
            for is_correct in result_rx {
                if is_correct {
                    result.correct_moves += 1;
                }
                result.completed_positions += 1;

                let progress = result.completed_positions as f64 / result.total_positions as f64;
                EVENT_BROKER.publish(Event::StsUpdate(StsUpdate {
                    config_hash: result.config_hash,
                    progress,
                    score: result.correct_moves,
                    total: result.total_positions,
                    elo: None,
                }));

                // Save progress every 10 positions
                if result.completed_positions % 10 == 0 {
                    let json = serde_json::to_string_pretty(&result).unwrap();
                    fs::write(&result_path, &json).expect("Failed to save STS result");
                }
            }

            // Finalize and save the result
            let score_percentage = (result.correct_moves as f64 / result.total_positions as f64) * 100.0;
            result.elo = Some(44.523 * score_percentage - 242.85);

            EVENT_BROKER.publish(Event::StsUpdate(StsUpdate {
                config_hash: result.config_hash,
                progress: 1.0,
                score: result.correct_moves,
                total: result.total_positions,
                elo: result.elo,
            }));

            let json = serde_json::to_string_pretty(&result).unwrap();
            fs::write(&result_path, json).expect("Failed to save final STS result");

            println!("STS run completed for config hash: {}", result.config_hash);

            result
        });

        let final_result = handle.await.ok();

        // Release the lock after the task is complete
        RUNNING_STS_TESTS.lock().unwrap().remove(&config_hash);
        println!("Released STS lock for config hash: {config_hash}");

        final_result
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
