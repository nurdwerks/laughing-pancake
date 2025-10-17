use serde::{Deserialize, Serialize};
use shakmaty::zobrist::Zobrist64;
use std::collections::HashMap;

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct MctsNodeData {
    pub visits: u32,
    pub wins: f64,
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct MctsCache {
    table: HashMap<u64, MctsNodeData>,
}

impl MctsCache {
    pub fn new() -> Self {
        Self {
            table: HashMap::new(),
        }
    }

    pub fn probe(&self, hash: &Zobrist64) -> Option<MctsNodeData> {
        self.table.get(&hash.0).copied()
    }

    pub fn store(&mut self, hash: Zobrist64, data: MctsNodeData) {
        self.table.insert(hash.0, data);
    }
}