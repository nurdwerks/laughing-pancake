// src/game/search/pvs.rs

//! Principal Variation Search (PVS)
//!
//! This is a stub for the PVS algorithm. PVS is an optimization of alpha-beta
//! search that can be more efficient in practice. It assumes that the first
//! move checked is the best one and searches it with a full alpha-beta window.
//! Subsequent moves are then searched with a "zero window" (alpha = beta - 1)
//! to prove that they are worse than the first move. If a move is found to be
//! better, it is re-searched with a full window.
