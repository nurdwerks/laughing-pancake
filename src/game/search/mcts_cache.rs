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

    pub fn from_map(map: HashMap<u64, MctsNodeData>) -> Self {
        Self { table: map }
    }

    pub fn get_table(&self) -> &HashMap<u64, MctsNodeData> {
        &self.table
    }

    pub fn probe(&self, hash: &Zobrist64) -> Option<MctsNodeData> {
        self.table.get(&hash.0).copied()
    }

    pub fn store(&mut self, hash: Zobrist64, data: MctsNodeData) {
        self.table.insert(hash.0, data);
    }

    pub fn update(&mut self, hash: Zobrist64, wins: f64, visits: u32) {
        let entry = self.table.entry(hash.0).or_insert(MctsNodeData {
            wins: 0.0,
            visits: 0,
        });
        entry.wins += wins;
        entry.visits += visits;
    }
}