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
    /// The token a `POST /api/login` must present to receive a session
    /// cookie. `None`/empty means auth is unusably misconfigured (login
    /// fails closed) — `serve()` auto-generates and persists one on first
    /// start rather than leaving this unset
    /// (`docs/history/daemon-auth-token-cf-access/CONTEXT.md` D3).
    pub web_secret: Option<String>,
    /// Cloudflare Access team domain (e.g. `https://team.cloudflareaccess.com`).
    /// CF Access only activates when this AND `cf_access_aud` are both set
    /// (D5) — one without the other leaves it fully off, never a partial
    /// check.
    pub cf_access_team_domain: Option<String>,
    /// Cloudflare Access Application Audience tag. See `cf_access_team_domain`.
    pub cf_access_aud: Option<String>,
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
            // Bind all interfaces by default so the viewer is reachable from
            // other devices on the LAN (and from a browser when the daemon runs
            // on a remote host). The server has no auth; `serve()` prints a
            // non-loopback exposure warning at startup.
            host: "0.0.0.0".into(),
            hostname: None,
            open_browser_on_start: false,
            web_secret: None,
            cf_access_team_domain: None,
            cf_access_aud: None,
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
    fn default_host_binds_all_interfaces() {
        // Fresh installs must default to the LAN-reachable wildcard bind.
        assert_eq!(ServerConfig::default().host, "0.0.0.0");
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

    #[test]
    fn auth_fields_default_unset_and_roundtrip_when_set() {
        assert_eq!(ServerConfig::default().web_secret, None);
        assert_eq!(ServerConfig::default().cf_access_team_domain, None);
        assert_eq!(ServerConfig::default().cf_access_aud, None);

        let dir = std::env::temp_dir().join(format!("mdview-cfg-auth-{}", std::process::id()));
        let p = dir.join("config.toml");
        let mut c = Config::default();
        c.server.web_secret = Some("s3cret".into());
        c.server.cf_access_team_domain = Some("https://team.cloudflareaccess.com".into());
        c.server.cf_access_aud = Some("aud-tag".into());
        c.save_to(&p).unwrap();
        let loaded = Config::load_from(&p);
        assert_eq!(loaded.server.web_secret.as_deref(), Some("s3cret"));
        assert_eq!(
            loaded.server.cf_access_team_domain.as_deref(),
            Some("https://team.cloudflareaccess.com")
        );
        assert_eq!(loaded.server.cf_access_aud.as_deref(), Some("aud-tag"));
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn config_missing_auth_fields_entirely_loads_as_unset() {
        // A config.toml written before this feature existed has none of
        // these keys at all — must load cleanly with auth off, not fail.
        let dir = std::env::temp_dir().join(format!("mdview-cfg-preauth-{}", std::process::id()));
        let p = dir.join("config.toml");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(&p, "[server]\nport = 7700\nhost = \"0.0.0.0\"\n").unwrap();
        let loaded = Config::load_from(&p);
        assert_eq!(loaded.server.web_secret, None);
        assert_eq!(loaded.server.cf_access_team_domain, None);
        std::fs::remove_dir_all(&dir).ok();
    }
}
