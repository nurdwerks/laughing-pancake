// src/game/search/lmr.rs

//! Late Move Reductions (LMR)
//!
//! This is a stub for the LMR algorithm. LMR is a search optimization technique
//! based on the idea that moves ordered later in the move list are less likely
//! to be good. Therefore, these moves can be searched with a reduced depth.
//! If a late move turns out to be better than expected (i.e., it causes a
//! beta cutoff), it can be re-searched at full depth. This can save a
//! significant amount of time in the search.
