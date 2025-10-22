#![cfg_attr(test, allow(dead_code))]

use crate::game::search::{MoveTreeNode, SearchConfig};
use crate::game::search::evaluation_cache::EvaluationCache;
use crate::game::search::{PvsSearcher, Searcher};
use crossbeam_channel::{Receiver, Sender};
use lazy_static::lazy_static;
use num_cpus;
use serde::Serialize;
use shakmaty::{san::San, Chess, Move};
use std::sync::{Arc, Mutex};
use std::thread;
use tokio::sync::oneshot;

#[derive(Debug, Clone, Serialize)]
pub enum Status {
    Idle,
    Busy(String), // The string will describe the job
}

#[derive(Debug, Clone, Serialize)]
pub struct WorkerStatus {
    pub id: usize,
    pub status: Status,
}

/// Represents a unit of work that can be processed by the worker pool.
#[derive(Debug)]
pub enum Job {
    /// A job to find the best move for a given chess position.
    FindBestMove {
        pos: Chess,
        config: SearchConfig,
        // Channel to send the result (best move, score, search tree) back.
        result_tx: oneshot::Sender<(Option<Move>, i32, Option<MoveTreeNode>)>,
    },
    /// A job to evaluate a single position from the Strategic Test Suite (STS).
    EvaluateStsPosition {
        pos: Chess,
        best_move_san: String,
        config: SearchConfig,
        // Channel to send the boolean result (correct move or not) back.
        result_tx: Sender<bool>,
    },
}

// A global, thread-safe FIFO queue for jobs.
lazy_static! {
    static ref JOB_QUEUE: (Sender<Job>, Receiver<Job>) = crossbeam_channel::unbounded();
    pub static ref WORKER_STATUSES: Arc<Mutex<Vec<WorkerStatus>>> =
        Arc::new(Mutex::new(Vec::new()));
}

/// Pushes a new job onto the global job queue.
pub fn push_job(job: Job) {
    JOB_QUEUE.0.send(job).expect("Failed to send job to queue. The receiver may have been dropped.");
}

/// Manages a pool of worker threads that process jobs from the queue.
pub struct WorkerPool {
    workers: Vec<thread::JoinHandle<()>>,
}

impl WorkerPool {
    /// Creates a new WorkerPool, spawning a worker thread for each logical CPU core.
    pub fn new() -> Self {
        let num_threads = num_cpus::get();
        let mut workers = Vec::with_capacity(num_threads);

        {
            let mut statuses = WORKER_STATUSES.lock().unwrap();
            for i in 0..num_threads {
                statuses.push(WorkerStatus { id: i, status: Status::Idle });
            }
        }

        for id in 0..num_threads {
            let job_rx = JOB_QUEUE.1.clone();

            let handle = thread::spawn(move || {
                let mut searcher =
                    PvsSearcher::with_shared_cache(Arc::new(Mutex::new(EvaluationCache::new())));

                while let Ok(job) = job_rx.recv() {
                    let job_description = format!("{:?}", job);

                    {
                        let mut statuses = WORKER_STATUSES.lock().unwrap();
                        if let Some(status) = statuses.get_mut(id) {
                            status.status = Status::Busy(job_description);
                        }
                    }

                    match job {
                        Job::FindBestMove {
                            pos,
                            config,
                            result_tx,
                        } => {
                            let (best_move, score, tree) =
                                searcher.search(&pos, config.search_depth, &config);
                            let _ = result_tx.send((best_move, score, tree));
                        }
                        Job::EvaluateStsPosition {
                            pos,
                            best_move_san,
                            config,
                            result_tx,
                        } => {
                            let (best_move, _, _) =
                                searcher.search(&pos, config.search_depth, &config);

                            let is_correct = if let Some(m) = best_move {
                                let san = San::from_move(&pos, m);
                                san.to_string() == best_move_san
                            } else {
                                false
                            };
                            let _ = result_tx.send(is_correct);
                        }
                    }

                    {
                        let mut statuses = WORKER_STATUSES.lock().unwrap();
                        if let Some(status) = statuses.get_mut(id) {
                            status.status = Status::Idle;
                        }
                    }
                }
            });

            workers.push(handle);
        }

        WorkerPool { workers }
    }
}
