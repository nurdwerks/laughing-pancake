// src/game/search/evaluation_cache.rs

pub use shakmaty::zobrist::Zobrist64;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use crate::constants::EVALUATION_CACHE_SIZE;

mod zobrist_serde {
    use serde::{self, Deserializer, Serializer, Deserialize};
    use shakmaty::zobrist::Zobrist64;

    pub fn serialize<S>(zobrist: &Zobrist64, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_u64(zobrist.0)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Zobrist64, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = u64::deserialize(deserializer)?;
        Ok(Zobrist64(value))
    }
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct CacheEntry {
    #[serde(with = "zobrist_serde")]
    pub hash: Zobrist64,
    pub score: i32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EvaluationCache {
    table: HashMap<u64, i32>,
    order: VecDeque<u64>,
}

impl EvaluationCache {
    #[cfg_attr(test, allow(dead_code))]
    pub fn new() -> Self {
        Self {
            table: HashMap::with_capacity(EVALUATION_CACHE_SIZE),
            order: VecDeque::with_capacity(EVALUATION_CACHE_SIZE),
        }
    }

    pub fn probe(&self, hash: &Zobrist64) -> Option<i32> {
        self.table.get(&hash.0).copied()
    }

    pub fn store(&mut self, entry: CacheEntry) {
        if self.table.len() >= EVALUATION_CACHE_SIZE {
            if let Some(oldest_hash) = self.order.pop_front() {
                self.table.remove(&oldest_hash);
            }
        }
        self.table.insert(entry.hash.0, entry.score);
        self.order.push_back(entry.hash.0);
    }
}