// src/event/mod.rs

use crate::ga::Match;
use actix::Message;
use once_cell::sync::Lazy;
use serde::Serialize;
use shakmaty::{fen::Fen, Chess, Setup};
use std::collections::HashMap;
use tokio::sync::broadcast;

/// This struct contains the entire state of the application that the web UI needs to render.
#[derive(Clone, Debug, Serialize)]
pub struct WebsocketState {
    // System info
    pub cpu_usage: f32,
    pub memory_usage: u64,
    pub total_memory: u64,
    // Evolution state
    pub evolution_log: Vec<String>,
    pub evolution_current_generation: u32,
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
#[derive(Clone, Debug, Message)]
#[rtype(result = "()")]
pub enum Event {
    WebsocketStateUpdate(WebsocketState),
    // Events used by the TUI and backend logic
    TournamentStart(usize, usize),
    GenerationStarted(u32),
    MatchStarted(usize, String, String),
    MatchCompleted(usize, Match),
    ThinkingUpdate(usize, String, i32),
    MovePlayed(usize, String, i32, Chess),
    StatusUpdate(String),
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