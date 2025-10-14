// src/game/search/futility.rs

//! Futility Pruning
//!
//! This is a stub for the futility pruning algorithm. This technique is used to
//! prune moves at the leaves of the search tree. If a move's potential gain
//! (e.g., the value of the piece being captured) is not enough to raise the
//! current evaluation score above alpha, then the move is considered "futile"
//! and can be pruned without a full search. This is most effective in the
//! later stages of the search.
