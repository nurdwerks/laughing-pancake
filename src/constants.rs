pub const NUM_ROUNDS: u32 = 7;
pub const STARTING_ELO: f64 = 1200.0;

// Genetic Algorithm
pub const POPULATION_SIZE: usize = 100;
pub const MUTATION_CHANCE: f64 = 0.05; // 5% chance for each parameter to mutate

// Knight Evaluation
pub const OUTPOST_BONUS: i32 = 30;
pub const CENTRALIZATION_BONUS: i32 = 10;

// Threat Evaluation
pub const UNDEFENDED_PIECE_BONUS_FACTOR: i32 = 10; // 10% of piece value
pub const GOOD_TRADE_BONUS_FACTOR: i32 = 5;      // 5% of piece value
pub const TACTICAL_PRESSURE_BONUS: i32 = 2;

// Rook Evaluation
pub const ROOK_OPEN_FILE_BONUS: i32 = 20;
pub const ROOK_SEMI_OPEN_FILE_BONUS: i32 = 10;
pub const SEVENTH_RANK_BONUS: i32 = 25;

// Bishop Evaluation
pub const BAD_BISHOP_PENALTY: i32 = -10;

// Initiative Evaluation
pub const THREAT_ON_MINOR_PIECE_BONUS: i32 = 10;
pub const THREAT_ON_MAJOR_PIECE_BONUS: i32 = 25;

// --- Piece values ---
pub const PAWN_VALUE: i32 = 100;
pub const KNIGHT_VALUE: i32 = 320;
pub const BISHOP_VALUE: i32 = 330;
pub const ROOK_VALUE: i32 = 500;
pub const QUEEN_VALUE: i32 = 900;

// Constants for game phase calculation
pub const QUEEN_PHASE_VAL: i32 = 4;
pub const ROOK_PHASE_VAL: i32 = 2;
pub const BISHOP_PHASE_VAL: i32 = 1;
pub const KNIGHT_PHASE_VAL: i32 = 1;
pub const TOTAL_PHASE: i32 =
    (QUEEN_PHASE_VAL * 2) + (ROOK_PHASE_VAL * 4) + (BISHOP_PHASE_VAL * 4) + (KNIGHT_PHASE_VAL * 4);

// Pawn Structure Evaluation
pub const PAWN_CHAIN_BONUS: i32 = 10;
pub const RAM_PENALTY: i32 = -5;
pub const CANDIDATE_PASSED_PAWN_BONUS: i32 = 15;

// Mobility Evaluation
pub const KNIGHT_MOBILITY_BONUS: i32 = 4;
pub const BISHOP_MOBILITY_BONUS: i32 = 5;
pub const ROOK_MOBILITY_BONUS: i32 = 2;
pub const QUEEN_MOBILITY_BONUS: i32 = 1;

// Development Evaluation
pub const DEVELOPMENT_BONUS_MINOR: i32 = 10;
pub const EARLY_QUEEN_MOVE_PENALTY: i32 = 15;

// Space Evaluation
pub const SPACE_PER_SQUARE_BONUS: i32 = 2;
pub const CENTER_CONTROL_BONUS: i32 = 5;

// Search
pub const MATE_SCORE: i32 = 1_000_000;

// Match Settings
pub const ENABLE_MOVE_LIMIT: bool = false;