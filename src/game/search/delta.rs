// src/game/search/delta.rs

//! Delta Pruning
//!
//! This is a stub for the delta pruning algorithm. Delta pruning is a forward
//! pruning technique similar to futility pruning, often used in quiescence search.
//! It is based on the idea that if the material difference between the two sides
//! is very large, then some moves (like captures of minor pieces) might not be
//! enough to change the outcome of the evaluation. If a move's potential material
//! gain plus a "delta" margin is still not enough to improve the score, it can
//! be pruned.
