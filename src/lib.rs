#![warn(missing_docs)]

//! history-flow — History Flow visualization for Wikipedia and Git.
//!
//! This library provides the three-stage pipeline:
//! - import: fetch revisions from Wikipedia or Git
//! - attribution: compute per-line author provenance
//! - visualize: emit Vega-Lite JSON specs

pub mod attribution;
pub mod config;
pub mod import;
pub mod visualize;
pub mod web;
