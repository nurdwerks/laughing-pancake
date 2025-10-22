// src/game/search/evaluation_cache.rs

pub use shakmaty::zobrist::Zobrist64;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

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
}

impl EvaluationCache {
    #[cfg_attr(test, allow(dead_code))]
    pub fn new() -> Self {
        Self {
            table: HashMap::new(),
        }
    }

    pub fn probe(&self, hash: &Zobrist64) -> Option<i32> {
        self.table.get(&hash.0).copied()
    }

    pub fn store(&mut self, entry: CacheEntry) {
        self.table.insert(entry.hash.0, entry.score);
    }
}