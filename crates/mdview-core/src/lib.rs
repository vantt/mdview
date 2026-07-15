//! mdview-core — domain + application + adapters for the mdview markdown server.
//!
//! Dependency rule (PRD §7.4): this crate never depends on Axum/Tauri. Adapters
//! (SQLite, notify) live here behind ports; the HTTP/MCP/CLI wiring is in the
//! `mdview` binary crate.

pub mod config;
pub mod daemon;
pub mod domain;
pub mod engine;
pub mod error;
pub mod indexer;
pub mod link_resolver;
pub mod render;
pub mod repository;

pub use config::Config;
pub use engine::Engine;
pub use error::{Error, Result};
pub use repository::SqliteStore;
