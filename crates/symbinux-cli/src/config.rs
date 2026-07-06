//! Optional user configuration, read from
//! `$XDG_CONFIG_HOME/symbinux/config.toml` (Linux/macOS) or
//! `%APPDATA%\symbinux\config.toml` (Windows). Every field is optional and the
//! loader never fails — a missing or malformed file simply yields defaults.
//!
//! Example `config.toml`:
//! ```toml
//! default_port = "/dev/nokia_fbus"
//! ack_timeout_ms = 400
//! retries = 3
//! log_level = "warn"
//! ```

use std::path::PathBuf;
use std::time::Duration;

use serde::Deserialize;
use symbinux_transport::ExchangeConfig;

#[derive(Debug, Default, Deserialize)]
pub struct Config {
    /// Serial port used when a command is given no `--port`.
    pub default_port: Option<String>,
    /// Per-attempt ACK timeout in milliseconds.
    pub ack_timeout_ms: Option<u64>,
    /// Number of retransmissions.
    pub retries: Option<u32>,
    /// Default `log` filter (e.g. "debug") when `RUST_LOG` is unset.
    pub log_level: Option<String>,
}

impl Config {
    /// Load the config file if present; otherwise return defaults. Never fails.
    pub fn load() -> Self {
        match config_path().and_then(|p| std::fs::read_to_string(p).ok()) {
            Some(text) => toml::from_str(&text).unwrap_or_default(),
            None => Self::default(),
        }
    }

    /// Build the transport exchange config, applying any overrides.
    pub fn exchange(&self) -> ExchangeConfig {
        let mut cfg = ExchangeConfig::default();
        if let Some(ms) = self.ack_timeout_ms {
            cfg.ack_timeout = Duration::from_millis(ms);
        }
        if let Some(r) = self.retries {
            cfg.retries = r;
        }
        cfg
    }
}

fn config_dir() -> Option<PathBuf> {
    if let Some(x) = std::env::var_os("XDG_CONFIG_HOME") {
        return Some(PathBuf::from(x));
    }
    if let Some(h) = std::env::var_os("HOME") {
        return Some(PathBuf::from(h).join(".config"));
    }
    std::env::var_os("APPDATA").map(PathBuf::from) // Windows fallback
}

fn config_path() -> Option<PathBuf> {
    config_dir().map(|d| d.join("symbinux").join("config.toml"))
}
