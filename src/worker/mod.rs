#![cfg_attr(test, allow(dead_code))]

use crate::game::search::{MoveTreeNode, SearchConfig, SearchAlgorithm};
use crate::game::search::evaluation_cache::EvaluationCache;
use crate::game::search::{mcts::MctsSearcher, PvsSearcher, Searcher};
use crossbeam_channel::{Receiver, Sender};
use lazy_static::lazy_static;
use serde::Serialize;
use shakmaty::{Chess, Move};
use std::panic::{self, AssertUnwindSafe};
use std::sync::{Arc, Mutex};
use std::thread;
use tokio::sync::oneshot;

pub type SearchResult = (Option<Move>, i32, Option<MoveTreeNode>, Option<String>);

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
        // Channel to send the result (best move, score, search tree, stats) back.
        result_tx: oneshot::Sender<SearchResult>,
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
pub struct WorkerPool;

impl WorkerPool {
    /// Creates a new WorkerPool, spawning a worker thread for each logical CPU core.
    pub fn new() -> Self {
        let num_threads = num_cpus::get();

        {
            let mut statuses = WORKER_STATUSES.lock().unwrap();
            for i in 0..num_threads {
                statuses.push(WorkerStatus { id: i, status: Status::Idle });
            }
        }

        for id in 0..num_threads {
            let job_rx = JOB_QUEUE.1.clone();

            let _handle = thread::spawn(move || {
                let mut pvs_searcher =
                    PvsSearcher::with_shared_cache(Arc::new(Mutex::new(EvaluationCache::new())));
                let mut mcts_searcher = MctsSearcher::new();

                while let Ok(job) = job_rx.recv() {
                    let job_description = format!("{job:?}");

                    {
                        let mut statuses = WORKER_STATUSES.lock().unwrap();
                        if let Some(status) = statuses.get_mut(id) {
                            status.status = Status::Busy(job_description);
                        }
                    }

                    let (pos, config, result_tx) = match job {
                        Job::FindBestMove {
                            pos,
                            config,
                            result_tx,
                        } => (pos, config, result_tx),
                    };

                    let result = panic::catch_unwind(AssertUnwindSafe(|| {
                        match config.search_algorithm {
                            SearchAlgorithm::Pvs => pvs_searcher.search(
                                &pos,
                                config.search_depth,
                                &config,
                                true,
                                false,
                            ),
                            SearchAlgorithm::Mcts => mcts_searcher.search(
                                &pos,
                                config.search_depth,
                                &config,
                                true,
                                false,
                            ),
                        }
                    }));

                    match result {
                        Ok(search_result) => {
                            let _ = result_tx.send(search_result);
                        }
                        Err(panic) => {
                            let panic_info = if let Some(s) = panic.downcast_ref::<&'static str>() {
                                *s
                            } else if let Some(s) = panic.downcast_ref::<String>() {
                                &s[..]
                            } else {
                                "Box<dyn Any>"
                            };
                            println!("MCTS task errored: {panic_info}");
                            pvs_searcher = PvsSearcher::with_shared_cache(Arc::new(Mutex::new(
                                EvaluationCache::new(),
                            )));
                            mcts_searcher = MctsSearcher::new();
                            let _ = result_tx.send((
                                None,
                                0,
                                None,
                                Some(format!("Worker panicked: {panic_info}")),
                            ));
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

            // The handle is intentionally detached here. The worker threads will run for the
            // lifetime of the application.
        }

        WorkerPool
    }
}
