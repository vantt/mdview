//! Config (`~/.mdview/config.toml`). Atomic write, resilient load (corrupt → default).
//! Mirrors PRD §10.

use crate::error::{Error, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub server: ServerConfig,
    pub mcp: McpConfig,
    pub indexing: IndexingConfig,
    pub renderer: RendererConfig,
    pub search: SearchConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ServerConfig {
    pub port: u16,
    pub host: String,
    /// Optional display hostname. When set, rendered view URLs use this
    /// instead of `host`/the daemon's bind address; the bind/connect
    /// address itself is unaffected.
    #[serde(alias = "host_name")]
    pub hostname: Option<String>,
    pub open_browser_on_start: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct McpConfig {
    pub enabled: bool,
    pub transport: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct IndexingConfig {
    pub debounce_ms: u64,
    pub max_file_size_mb: u64,
    pub exclude_patterns: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct RendererConfig {
    pub theme: String,
    pub syntax_highlight_theme: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct SearchConfig {
    pub enable_fts: bool,
    pub enable_semantic: bool,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            port: 7700,
            host: "127.0.0.1".into(),
            hostname: None,
            open_browser_on_start: false,
        }
    }
}
impl Default for McpConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            transport: "stdio".into(),
        }
    }
}
impl Default for IndexingConfig {
    fn default() -> Self {
        Self {
            debounce_ms: 200,
            max_file_size_mb: 10,
            exclude_patterns: vec![
                ".git".into(),
                "node_modules".into(),
                ".venv".into(),
                "target".into(),
                "dist".into(),
            ],
        }
    }
}
impl Default for RendererConfig {
    fn default() -> Self {
        Self {
            theme: "system".into(),
            syntax_highlight_theme: "github-dark".into(),
        }
    }
}
impl Default for SearchConfig {
    fn default() -> Self {
        Self {
            enable_fts: true,
            enable_semantic: false,
        }
    }
}
/// `~/.mdview/` — the app data directory (created on demand).
pub fn data_dir() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".mdview")
}

pub fn config_path() -> PathBuf {
    data_dir().join("config.toml")
}

pub fn registry_db_path() -> PathBuf {
    data_dir().join("registry.db")
}

pub fn daemon_lock_path() -> PathBuf {
    data_dir().join("daemon.lock")
}

impl Config {
    /// Load config; a missing or corrupt file resolves to defaults (never panics).
    pub fn load() -> Self {
        Self::load_from(&config_path())
    }

    pub fn load_from(path: &Path) -> Self {
        match std::fs::read_to_string(path) {
            Ok(text) => toml::from_str(&text).unwrap_or_else(|e| {
                tracing::warn!("config parse failed ({e}); using defaults");
                Config::default()
            }),
            Err(_) => Config::default(),
        }
    }

    /// Atomic write: serialize → temp file → rename (survives crash mid-write).
    pub fn save(&self) -> Result<()> {
        self.save_to(&config_path())
    }

    pub fn save_to(&self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let text =
            toml::to_string_pretty(self).map_err(|e| Error::Config(format!("serialize: {e}")))?;
        write_atomic(path, text.as_bytes())
    }
}

/// Atomic file write via temp-in-same-dir + rename. Shared by config & registry snapshots.
pub fn write_atomic(path: &Path, bytes: &[u8]) -> Result<()> {
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    std::fs::create_dir_all(parent)?;
    let tmp = parent.join(format!(
        ".{}.tmp{}",
        path.file_name().and_then(|s| s.to_str()).unwrap_or("f"),
        std::process::id()
    ));
    std::fs::write(&tmp, bytes)?;
    std::fs::rename(&tmp, path)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn corrupt_config_falls_back_to_default() {
        let dir = std::env::temp_dir().join(format!("mdview-cfg-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join("config.toml");
        std::fs::write(&p, "this is not = valid : toml ][").unwrap();
        let c = Config::load_from(&p);
        assert_eq!(c.server.port, 7700);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn roundtrip_atomic_save_load() {
        let dir = std::env::temp_dir().join(format!("mdview-cfg2-{}", std::process::id()));
        let p = dir.join("config.toml");
        let mut c = Config::default();
        c.server.port = 9999;
        c.save_to(&p).unwrap();
        let loaded = Config::load_from(&p);
        assert_eq!(loaded.server.port, 9999);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn hostname_defaults_to_none_and_roundtrips_when_set() {
        assert_eq!(ServerConfig::default().hostname, None);

        let dir = std::env::temp_dir().join(format!("mdview-cfg3-{}", std::process::id()));
        let p = dir.join("config.toml");
        let mut c = Config::default();
        c.server.hostname = Some("my-machine.local".into());
        c.save_to(&p).unwrap();
        let loaded = Config::load_from(&p);
        assert_eq!(loaded.server.hostname.as_deref(), Some("my-machine.local"));
        std::fs::remove_dir_all(&dir).ok();
    }
}
