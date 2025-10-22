// src/event/mod.rs

#![cfg_attr(test, allow(dead_code))]

use actix::Message;
use once_cell::sync::Lazy;
use serde::Serialize;
use shakmaty::{fen::Fen, Chess};
use std::collections::HashMap;
use tokio::sync::broadcast;

#[derive(Debug, Clone, Serialize)]
pub struct ComponentState {
    pub label: String,
    pub temperature: f32,
}

use crate::server::StsRunResponse;

// This struct contains the entire state of the application that the web UI needs to render.
use crate::worker::WorkerStatus;
use serde::Deserialize;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum SelectionAlgorithm {
    SwissTournament,
    StsScore,
}

#[derive(Clone, Debug, Serialize)]
pub struct StsLeaderboardEntry {
    pub individual_id: usize,
    pub progress: f64,
    pub elo: Option<f64>,
}

#[derive(Clone, Debug, Serialize)]
pub struct WebsocketState {
    pub git_hash: String,
    // System info
    pub cpu_usage: f32,
    pub memory_usage: u64,
    pub total_memory: u64,
    pub components: Vec<ComponentState>,
    // Evolution state
    pub evolution_current_generation: u32,
    pub evolution_current_round: usize,
    pub evolution_total_rounds: u32,
    pub evolution_matches_completed: usize,
    pub evolution_total_matches: usize,
    pub active_matches: HashMap<usize, ActiveMatchState>,
    pub worker_statuses: Vec<WorkerStatus>,
    pub selection_algorithm: SelectionAlgorithm,
    pub sts_leaderboard: Vec<StsLeaderboardEntry>,
}

#[derive(Clone, Debug, Serialize)]
pub struct ActiveMatchState {
    pub board: String, // FEN representation of the board
    pub white_player: String,
    pub black_player: String,
    pub san: String,
    pub eval: i32,
    pub material: i32,
}

/// Defines all possible events that can occur in the application.
#[derive(Clone, Debug, Message, Serialize)]
#[rtype(result = "()")]
#[serde(tag = "type", content = "payload")]
pub enum WsMessage {
    State(WebsocketState),
    Log(String),
    Sts(StsUpdate),
    StsStarted(StsRunResponse),
}

#[derive(Clone, Debug)]
pub struct MatchResult {
    pub white_player_name: String,
    pub black_player_name: String,
    pub result: String,
}

#[derive(Clone, Debug)]
pub struct GenerationStats {
    pub generation_index: u32,
    pub num_matches: usize,
    pub white_wins: usize,
    pub black_wins: usize,
    pub draws: usize,
    pub top_elo: f64,
    pub average_elo: f64,
    pub lowest_elo: f64,
}

#[derive(Clone, Debug, Serialize)]
pub struct StsUpdate {
    pub config_hash: u64,
    pub progress: f64,
    pub score: usize,
    pub total: usize,
    pub elo: Option<f64>,
}

/// Defines all possible events that can occur in the application.
#[derive(Clone, Debug, Message)]
#[rtype(result = "()")]
pub enum Event {
    WebsocketStateUpdate(WebsocketState),
    StsUpdate(StsUpdate),
    // Events used by the TUI and backend logic
    StsModeActive(SelectionAlgorithm),
    StsProgress(StsLeaderboardEntry),
    TournamentStart(usize, usize, usize),
    GenerationStarted(u32),
    GenerationComplete(GenerationStats),
    MatchStarted(usize, String, String),
    MatchCompleted(usize, MatchResult),
    ThinkingUpdate(usize, String, i32),
    MovePlayed(usize, String, i32, Chess),
    StatusUpdate(String),
    LogUpdate(String),
    Panic(String),
    ForceQuit,
    ResetSimulation,
}

impl From<&Chess> for ActiveMatchState {
    fn from(chess: &Chess) -> Self {
        let fen: Fen = Fen::from_position(chess, shakmaty::EnPassantMode::Legal);
        ActiveMatchState {
            board: fen.to_string(),
            white_player: String::new(),
            black_player: String::new(),
            san: String::new(),
            eval: 0,
            material: 0,
        }
    }
}

pub struct EventBroker {
    sender: broadcast::Sender<Event>,
}

impl EventBroker {
    pub fn new() -> Self {
        let (sender, _) = broadcast::channel(1024);
        Self { sender }
    }

    pub fn publish(&self, event: Event) {
        // Ignore errors, as it's fine if there are no subscribers
        let _ = self.sender.send(event);
    }

    pub fn subscribe(&self) -> broadcast::Receiver<Event> {
        self.sender.subscribe()
    }
}

impl Default for EventBroker {
    fn default() -> Self {
        Self::new()
    }
}

pub static EVENT_BROKER: Lazy<EventBroker> = Lazy::new(EventBroker::new);