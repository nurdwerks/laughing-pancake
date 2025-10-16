// src/game/search/evaluation_cache.rs

pub use shakmaty::zobrist::Zobrist64;
use std::collections::HashMap;

#[derive(Clone, Copy, Debug)]
pub struct CacheEntry {
    pub hash: Zobrist64,
    pub score: i32,
}

pub struct EvaluationCache {
    table: HashMap<Zobrist64, CacheEntry>,
}

impl EvaluationCache {
    pub fn new() -> Self {
        Self {
            table: HashMap::new(),
        }
    }

    pub fn probe(&self, hash: &Zobrist64) -> Option<i32> {
        self.table.get(hash).map(|entry| entry.score)
    }

    pub fn store(&mut self, entry: CacheEntry) {
        self.table.insert(entry.hash, entry);
    }
}