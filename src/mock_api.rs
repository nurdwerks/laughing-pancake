//! src/mock_api.rs
// This module provides mock data for the API endpoints and WebSocket messages.

use crate::{
    event::{StsLeaderboardEntry, WebsocketState, SelectionAlgorithm},
    ga::Individual,
    game::search::SearchConfig,
    server::{ApiGenerationDetails, ApiIndividual, GenerationSummary},
};
use lazy_static::lazy_static;

// Scenario C: A fresh server state with no historical data.
lazy_static! {
    pub static ref MOCK_WEBSOCKET_STATE_C: WebsocketState = WebsocketState {
        git_hash: "mock_hash".to_string(),
        ..Default::default()
    };
    pub static ref MOCK_GENERATIONS_C: Vec<GenerationSummary> = vec![];
}

// Scenario B: Several completed generations and no active generation.
lazy_static! {
    pub static ref MOCK_WEBSOCKET_STATE_B: WebsocketState = WebsocketState {
        git_hash: "mock_hash".to_string(),
        evolution_current_generation: 2,
        ..Default::default()
    };
    pub static ref MOCK_GENERATIONS_B: Vec<GenerationSummary> = vec![
        GenerationSummary {
            generation_index: 0,
            selection_algorithm: SelectionAlgorithm::SwissTournament,
            num_individuals: 10,
            num_matches: 0,
            white_wins: 0,
            black_wins: 0,
            draws: 0,
            top_elo: 0.0,
            average_elo: 0.0,
            lowest_elo: 0.0,
        },
        GenerationSummary {
            generation_index: 1,
            selection_algorithm: SelectionAlgorithm::StsScore,
            num_individuals: 10,
            num_matches: 0,
            white_wins: 0,
            black_wins: 0,
            draws: 0,
            top_elo: 0.0,
            average_elo: 0.0,
            lowest_elo: 0.0,
        },
    ];
    pub static ref MOCK_GENERATION_DETAILS_B0: ApiGenerationDetails = ApiGenerationDetails {
        generation_index: 0,
        round: 7,
        population: (0..10)
            .map(|i| ApiIndividual {
                id: i,
                elo: 1200.0,
                config_hash: i as u64,
                config: SearchConfig::default(),
            })
            .collect(),
        matches: vec![],
        sts_results: None,
    };
    pub static ref MOCK_GENERATION_DETAILS_B1: ApiGenerationDetails = ApiGenerationDetails {
        generation_index: 1,
        round: 0,
        population: (0..10)
            .map(|i| ApiIndividual {
                id: i,
                elo: 1200.0 + (i as f64 * 10.0),
                config_hash: i as u64,
                config: SearchConfig::default(),
            })
            .collect(),
        matches: vec![],
        sts_results: Some(vec![]),
    };
    pub static ref MOCK_INDIVIDUAL_B0_0: Individual = Individual {
        id: 0,
        config: SearchConfig::default(),
        elo: 1200.0,
    };
}

use crate::ga::GenerationConfig;

// Scenario A: Several completed generations (mix of Swiss and STS) and an STS generation currently in progress.
lazy_static! {
    pub static ref MOCK_CONFIG_B0: GenerationConfig = GenerationConfig {
        selection_algorithm: SelectionAlgorithm::SwissTournament,
    };
    pub static ref MOCK_CONFIG_B1: GenerationConfig = GenerationConfig {
        selection_algorithm: SelectionAlgorithm::StsScore,
    };

    pub static ref MOCK_WEBSOCKET_STATE_A: WebsocketState = WebsocketState {
        git_hash: "mock_hash".to_string(),
        evolution_current_generation: 2,
        selection_algorithm: SelectionAlgorithm::StsScore,
        sts_leaderboard: (0..10)
            .map(|i| StsLeaderboardEntry {
                individual_id: i,
                progress: (i * 10) as f64,
                elo: Some(1200.0 + (i as f64 * 5.0)),
            })
            .collect(),
        ..Default::default()
    };
    pub static ref MOCK_GENERATIONS_A: Vec<GenerationSummary> = MOCK_GENERATIONS_B.to_vec();
    pub static ref MOCK_GENERATION_DETAILS_A0: ApiGenerationDetails = MOCK_GENERATION_DETAILS_B0.clone();
    pub static ref MOCK_GENERATION_DETAILS_A1: ApiGenerationDetails = MOCK_GENERATION_DETAILS_B1.clone();
}
