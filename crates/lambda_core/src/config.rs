//! `config.json` — connection + timer settings (architecture §4.1).

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::defaults;
use crate::persist_io::{atomic_write_json, load_json_file, LoadOutcome};

pub const CONFIG_SCHEMA_VERSION: u32 = 1;
pub const CONFIG_FILE_NAME: &str = "config.json";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ConfigFile {
    pub schema_version: u32,
    pub host: String,
    pub port: u16,
    pub unit_id: u8,
    pub poll_interval_secs: u32,
    pub cyclic_write_interval_secs: u32,
    pub connect_timeout_ms: u32,
    pub request_timeout_ms: u32,
}

impl Default for ConfigFile {
    fn default() -> Self {
        Self {
            schema_version: CONFIG_SCHEMA_VERSION,
            host: defaults::HOST.to_string(),
            port: defaults::PORT,
            unit_id: defaults::UNIT_ID,
            poll_interval_secs: defaults::POLL_INTERVAL_SECS,
            cyclic_write_interval_secs: defaults::CYCLIC_WRITE_INTERVAL_SECS,
            connect_timeout_ms: defaults::CONNECT_TIMEOUT_MS,
            request_timeout_ms: defaults::REQUEST_TIMEOUT_MS,
        }
    }
}

impl ConfigFile {
    pub fn validate(&self) -> Result<(), String> {
        if self.schema_version == 0 {
            return Err("Ungültige schema_version: 0".into());
        }
        if self.host.trim().is_empty() {
            return Err("Host darf nicht leer sein".into());
        }
        if self.poll_interval_secs == 0 {
            return Err("poll_interval_secs muss > 0 sein".into());
        }
        if self.cyclic_write_interval_secs == 0 {
            return Err("cyclic_write_interval_secs muss > 0 sein".into());
        }
        Ok(())
    }
}

/// Load config from `dir/config.json`, falling back to defaults.
pub fn load_config(dir: &Path) -> (ConfigFile, Option<String>) {
    let path = dir.join(CONFIG_FILE_NAME);
    match load_json_file::<ConfigFile>(&path) {
        LoadOutcome::Missing => (ConfigFile::default(), None),
        LoadOutcome::Loaded(cfg) => match cfg.validate() {
            Ok(()) => (cfg, None),
            Err(msg) => (
                ConfigFile::default(),
                Some(format!("config.json ungültig ({msg}) — Defaults")),
            ),
        },
        LoadOutcome::Fallback { message } => (ConfigFile::default(), Some(message)),
    }
}

/// Atomically persist config under `dir/config.json`.
pub fn save_config(dir: &Path, cfg: &ConfigFile) -> Result<PathBuf, String> {
    cfg.validate()?;
    let path = dir.join(CONFIG_FILE_NAME);
    atomic_write_json(&path, cfg)?;
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_dir() -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("lambda-cfg-{nanos}"));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn defaults_match_locked_product() {
        let d = ConfigFile::default();
        assert_eq!(d.host, "10.0.0.6");
        assert_eq!(d.port, 502);
        assert_eq!(d.unit_id, 1);
        assert_eq!(d.poll_interval_secs, 5);
        assert_eq!(d.cyclic_write_interval_secs, 60);
    }

    #[test]
    fn roundtrip_temp_dir() {
        let dir = temp_dir();
        let mut cfg = ConfigFile::default();
        cfg.host = "192.168.1.50".into();
        save_config(&dir, &cfg).unwrap();
        let (loaded, warn) = load_config(&dir);
        assert!(warn.is_none());
        assert_eq!(loaded.host, "192.168.1.50");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn corrupt_falls_back() {
        let dir = temp_dir();
        fs::write(dir.join(CONFIG_FILE_NAME), "???").unwrap();
        let (cfg, warn) = load_config(&dir);
        assert_eq!(cfg, ConfigFile::default());
        assert!(warn.is_some());
        let _ = fs::remove_dir_all(&dir);
    }
}
