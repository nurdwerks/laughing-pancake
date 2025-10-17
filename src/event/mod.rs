// src/event/mod.rs

use actix::Message;
use once_cell::sync::Lazy;
use serde::Serialize;
use shakmaty::{fen::Fen, Chess};
use std::collections::HashMap;
use tokio::sync::broadcast;

#[derive(Debug, Clone, Serialize)]
pub struct CpuState {
    pub usage: f32,
}

#[derive(Debug, Clone, Serialize)]
pub struct ComponentState {
    pub label: String,
    pub temperature: f32,
}

// This struct contains the entire state of the application that the web UI needs to render.
#[derive(Clone, Debug, Serialize)]
pub struct WebsocketState {
    pub git_hash: String,
    pub graceful_shutdown: bool,
    // System info
    pub cpu_usage: f32,
    pub memory_usage: u64,
    pub total_memory: u64,
    pub cpus: Vec<CpuState>,
    pub components: Vec<ComponentState>,
    // Evolution state
    pub evolution_current_generation: u32,
    pub evolution_current_round: usize,
    pub evolution_matches_completed: usize,
    pub evolution_total_matches: usize,
    pub active_matches: HashMap<usize, ActiveMatchState>,
    pub evolution_workers: Vec<WorkerState>,
}

#[derive(Clone, Debug, Serialize)]
pub struct WorkerState {
    pub id: u64,
    pub name: String,
    pub elapsed_time: f64,
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
}

#[derive(Clone, Debug)]
pub struct MatchResult {
    pub white_player_name: String,
    pub black_player_name: String,
    pub white_new_elo: f64,
    pub black_new_elo: f64,
    pub result: String,
}

/// Defines all possible events that can occur in the application.
#[derive(Clone, Debug, Message)]
#[rtype(result = "()")]
pub enum Event {
    WebsocketStateUpdate(WebsocketState),
    // Events used by the TUI and backend logic
    TournamentStart(usize, usize, usize),
    GenerationStarted(u32),
    MatchStarted(usize, String, String),
    MatchCompleted(usize, MatchResult),
    ThinkingUpdate(usize, String, i32),
    MovePlayed(usize, String, i32, Chess),
    StatusUpdate(String),
    LogUpdate(String),
    Panic(String),
    RequestQuit,
    ForceQuit,
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
        let (sender, _) = broadcast::channel(100);
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