//! Core error type for mdview. Adapters map these to HTTP/MCP/CLI surfaces.

use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("project not found: {0}")]
    ProjectNotFound(String),

    #[error("file not found in index: {0}")]
    FileNotFound(String),

    #[error("path is outside any registered project root: {0}")]
    PathOutsideProject(PathBuf),

    #[error("invalid path: {0}")]
    InvalidPath(String),

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("database error: {0}")]
    Db(#[from] rusqlite::Error),

    #[error("config error: {0}")]
    Config(String),

    #[error("{0}")]
    Other(String),
}

pub type Result<T> = std::result::Result<T, Error>;
