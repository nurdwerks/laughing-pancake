// src/event/mod.rs

use crate::ga::{Match};
use shakmaty::{Chess};
use tokio::sync::broadcast;
use once_cell::sync::Lazy;
use std::fmt;

use actix::Message;

/// Defines all possible events that can occur in the application.
/// These events are published by the backend and subscribed to by the frontend.
#[derive(Clone, Message)]
#[rtype(result = "()")]
pub enum Event {
    TournamentStart(usize, usize), // Total matches, skipped matches
    GenerationStarted(u32),
    MatchStarted(usize, String, String), // Match index, White player name, Black player name
    MatchCompleted(usize, Match), // Match index, Match
    ThinkingUpdate(usize, String, i32),  // Match index, Thinking message, evaluation
    MovePlayed(usize, String, i32, Chess), // Match index, SAN of the move, material difference, new board position
    StatusUpdate(String),
    Panic(String),
    Quit,
}

/// The `EventBroker` is responsible for receiving events and broadcasting them to all subscribers.
/// It uses a `tokio::sync::broadcast` channel to allow multiple subscribers to listen for events.
pub struct EventBroker {
    sender: broadcast::Sender<Event>,
}

impl EventBroker {
    /// Creates a new `EventBroker`.
    /// The `channel` method returns a `Sender` and `Receiver`.
    /// The `Sender` is used to send events to the broker, and the `Receiver` is used to subscribe to events.
    pub fn new() -> Self {
        let (sender, _) = broadcast::channel(100);
        Self { sender }
    }

    /// Publishes an event to all subscribers.
    /// The `send` method returns the number of subscribers that received the event.
    pub fn publish(&self, event: Event) {
        if self.sender.send(event).is_err() {
            // This error occurs when there are no subscribers.
            // In our design, it's possible for the backend to start publishing events before the TUI or web server has subscribed.
            // Therefore, we can safely ignore this error.
        }
    }

    /// Creates a new subscriber to the event stream.
    /// The `subscribe` method returns a `Receiver` that can be used to receive events.
    pub fn subscribe(&self) -> broadcast::Receiver<Event> {
        self.sender.subscribe()
    }
}

impl Default for EventBroker {
    fn default() -> Self {
        Self::new()
    }
}

/// A global, lazily-initialized instance of the `EventBroker`.
/// This allows any part of the application to publish events without needing to pass around a reference to the broker.
pub static EVENT_BROKER: Lazy<EventBroker> = Lazy::new(EventBroker::new);

// Implement `Debug` for `Event` so it can be easily printed for logging and debugging.
impl fmt::Debug for Event {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Event::TournamentStart(total, skipped) => write!(f, "TournamentStart(total: {}, skipped: {})", total, skipped),
            Event::GenerationStarted(gen_index) => write!(f, "GenerationStarted(generation: {})", gen_index),
            Event::MatchStarted(id, white, black) => write!(f, "MatchStarted(id: {}, white: {}, black: {})", id, white, black),
            Event::MatchCompleted(id, game_match) => write!(f, "MatchCompleted(id: {}, match: {:?})", id, game_match),
            Event::ThinkingUpdate(id, pv, eval) => write!(f, "ThinkingUpdate(id: {}, pv: {}, eval: {})", id, pv, eval),
            Event::MovePlayed(id, san, material, _) => write!(f, "MovePlayed(id: {}, san: {}, material: {})", id, san, material),
            Event::StatusUpdate(msg) => write!(f, "StatusUpdate({})", msg),
            Event::Panic(msg) => write!(f, "Panic({})", msg),
        }
    }
}