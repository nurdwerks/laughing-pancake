// src/config.rs

use crate::game::search::SearchConfig;
use std::fs;
use std::io::{self, Write};
use std::path::Path;

const PROFILES_DIR: &str = "profiles";

pub fn save_profile(name: &str, config: &SearchConfig) -> io::Result<()> {
    let path = Path::new(PROFILES_DIR).join(format!("{}.json", name));
    let json = serde_json::to_string_pretty(config)?;
    fs::File::create(path)?.write_all(json.as_bytes())
}

pub fn load_profile(name: &str) -> io::Result<SearchConfig> {
    let path = Path::new(PROFILES_DIR).join(format!("{}.json", name));
    let json = fs::read_to_string(path)?;
    serde_json::from_str(&json).map_err(io::Error::from)
}

pub fn get_profiles() -> io::Result<Vec<String>> {
    let mut profiles = Vec::new();
    for entry in fs::read_dir(PROFILES_DIR)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_file() {
            if let Some(stem) = path.file_stem() {
                if let Some(name) = stem.to_str() {
                    profiles.push(name.to_string());
                }
            }
        }
    }
    Ok(profiles)
}
