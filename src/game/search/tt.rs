// src/game/search/tt.rs

use shakmaty::{Move};
pub use shakmaty::zobrist::Zobrist64;
use std::collections::HashMap;

#[derive(Clone, Copy, Debug)]
pub enum Bound {
    Exact,
    Lower,
    Upper,
}

#[derive(Clone, Copy, Debug)]
pub struct TTEntry {
    pub hash: Zobrist64,
    pub depth: u8,
    pub score: i32,
    pub bound: Bound,
    pub best_move: Option<Move>,
}

pub struct TranspositionTable {
    table: HashMap<Zobrist64, TTEntry>,
}

impl TranspositionTable {
    pub fn new() -> Self {
        Self {
            table: HashMap::new(),
        }
    }

    pub fn probe(&self, hash: &Zobrist64) -> Option<&TTEntry> {
        self.table.get(hash)
    }

    pub fn store(&mut self, entry: TTEntry) {
        self.table.insert(entry.hash, entry);
    }
}