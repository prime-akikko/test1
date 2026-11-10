//! Active-Setpoint store for 60 s cyclic Modbus rewrites (architecture §4.3 / §10.2).
//!
//! Persists to `active_setpoints.json` under the app data dir when a path is set.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::persist_io::{atomic_write_json, load_json_file, LoadOutcome};

/// Schema version for `active_setpoints.json` (architecture §4.3).
pub const ACTIVE_SETPOINTS_SCHEMA_VERSION: u32 = 1;

/// Default relative file name under the app config dir (M8 resolves the directory).
pub const ACTIVE_SETPOINTS_FILE_NAME: &str = "active_setpoints.json";

/// One value inside a write-group Active entry.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GroupValue {
    pub param_id: String,
    pub display_value: f64,
}

/// Active setpoint that the Cyclic-Write-Scheduler will rewrite every interval.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ActiveSetpoint {
    /// Catalog param id, or group anchor when `write_group_id` is set.
    pub param_id: String,
    /// When set, the whole write group is rewritten as one FC 0x10.
    pub write_group_id: Option<String>,
    /// Last confirmed display value (single-parameter entries).
    pub display_value: f64,
    /// Values for a group write; `None` for single-parameter entries.
    pub group_values: Option<Vec<GroupValue>>,
    /// Activation time (ISO-8601-ish wall clock string).
    pub activated_at: String,
    /// Last successful (re)write time, if any.
    pub last_write_at: Option<String>,
    /// Last rewrite error message (entry is kept on failure — F-43).
    pub last_error: Option<String>,
}

impl ActiveSetpoint {
    /// Stable key used in the store map (group id wins over param id).
    pub fn key(&self) -> &str {
        self.write_group_id
            .as_deref()
            .unwrap_or(self.param_id.as_str())
    }
}

/// On-disk document shape for M8 (`active_setpoints.json`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct ActiveSetpointsFile {
    pub schema_version: u32,
    pub entries: Vec<ActiveSetpoint>,
}

impl ActiveSetpointsFile {
    pub fn new(entries: Vec<ActiveSetpoint>) -> Self {
        Self {
            schema_version: ACTIVE_SETPOINTS_SCHEMA_VERSION,
            entries,
        }
    }
}

/// In-memory Active-Setpoint store with optional persistence path stub.
#[derive(Debug, Clone, Default)]
pub struct ActiveSetpointStore {
    entries: Vec<ActiveSetpoint>,
    /// Target path for M8 atomic writes. Unused in M5 beyond round-trip helpers.
    persist_path: Option<PathBuf>,
}

impl ActiveSetpointStore {
    pub fn new() -> Self {
        Self::default()
    }

    /// Stub: remember where M8 should persist (`…/active_setpoints.json`).
    pub fn with_persist_path(path: impl Into<PathBuf>) -> Self {
        Self {
            entries: Vec::new(),
            persist_path: Some(path.into()),
        }
    }

    pub fn persist_path(&self) -> Option<&Path> {
        self.persist_path.as_deref()
    }

    pub fn set_persist_path(&mut self, path: impl Into<PathBuf>) {
        self.persist_path = Some(path.into());
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn entries(&self) -> &[ActiveSetpoint] {
        &self.entries
    }

    pub fn get(&self, key: &str) -> Option<&ActiveSetpoint> {
        self.entries.iter().find(|e| e.key() == key)
    }

    pub fn get_mut(&mut self, key: &str) -> Option<&mut ActiveSetpoint> {
        self.entries.iter_mut().find(|e| e.key() == key)
    }

    /// Insert or replace by [`ActiveSetpoint::key`].
    pub fn upsert(&mut self, entry: ActiveSetpoint) {
        let key = entry.key().to_string();
        if let Some(existing) = self.get_mut(&key) {
            *existing = entry;
        } else {
            self.entries.push(entry);
        }
    }

    /// Remove by param id or write_group_id. Returns `true` if something was removed.
    pub fn remove(&mut self, key: &str) -> bool {
        let before = self.entries.len();
        self.entries.retain(|e| e.key() != key && e.param_id != key);
        before != self.entries.len()
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }

    /// Snapshot as the on-disk document model (ready for M8).
    pub fn to_file(&self) -> ActiveSetpointsFile {
        ActiveSetpointsFile::new(self.entries.clone())
    }

    /// Replace in-memory entries from a file document.
    pub fn replace_from_file(&mut self, file: ActiveSetpointsFile) {
        self.entries = file.entries;
    }

    /// Serialize to JSON string (tests / diagnostics).
    pub fn serialize_json(&self) -> Result<String, String> {
        serde_json::to_string_pretty(&self.to_file()).map_err(|e| e.to_string())
    }

    /// Deserialize JSON into memory (no filesystem read).
    pub fn load_from_json(&mut self, json: &str) -> Result<(), String> {
        let file: ActiveSetpointsFile = serde_json::from_str(json).map_err(|e| e.to_string())?;
        if file.schema_version == 0 {
            return Err("Ungültige schema_version: 0".into());
        }
        self.replace_from_file(file);
        Ok(())
    }

    /// Atomically persist to [`Self::persist_path`]. No-op (Ok) if path unset.
    pub fn save(&self) -> Result<(), String> {
        let Some(path) = self.persist_path.as_ref() else {
            return Ok(());
        };
        let file = self.to_file();
        atomic_write_json(path, &file)
    }

    /// Load from [`Self::persist_path`]. Missing → empty. Corrupt → clear + Err message.
    pub fn load(&mut self) -> Result<Option<String>, String> {
        let Some(path) = self.persist_path.clone() else {
            return Ok(None);
        };
        match load_json_file::<ActiveSetpointsFile>(&path) {
            LoadOutcome::Missing => {
                self.entries.clear();
                Ok(None)
            }
            LoadOutcome::Loaded(file) => {
                if file.schema_version == 0 {
                    self.entries.clear();
                    return Ok(Some(
                        "active_setpoints.json ungültig (schema_version 0) — leere Liste".into(),
                    ));
                }
                self.replace_from_file(file);
                Ok(None)
            }
            LoadOutcome::Fallback { message } => {
                self.entries.clear();
                Ok(Some(message))
            }
        }
    }

    /// Backward-compatible alias used by the cyclic scheduler.
    pub fn save_stub(&self) -> Result<(), String> {
        self.save()
    }

    /// Backward-compatible alias — prefer [`Self::load`].
    pub fn load_stub(&mut self) -> Result<(), String> {
        self.load().map(|_| ())
    }
}

/// Wall-clock timestamp string for Active entries (ISO-8601 UTC, second precision).
///
/// Uses `SystemTime` so FakeClock (monotonic Instant) stays independent. M8 may
/// switch to a richer clock source.
pub fn wall_now_iso8601() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    format_unix_secs_iso8601(secs)
}

/// Format unix seconds as `YYYY-MM-DDTHH:MM:SSZ` (UTC).
pub fn format_unix_secs_iso8601(mut secs: u64) -> String {
    // Civil date from unix days (Howard Hinnant algorithm).
    let days = (secs / 86_400) as i64;
    secs %= 86_400;
    let hours = secs / 3600;
    secs %= 3600;
    let mins = secs / 60;
    let secs = secs % 60;

    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };

    format!("{y:04}-{m:02}-{d:02}T{hours:02}:{mins:02}:{secs:02}Z")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn upsert_and_remove_by_param_or_group() {
        let mut store = ActiveSetpointStore::new();
        store.upsert(ActiveSetpoint {
            param_id: "hk.1.flow_setpoint".into(),
            write_group_id: None,
            display_value: 42.5,
            group_values: None,
            activated_at: "t0".into(),
            last_write_at: None,
            last_error: None,
        });
        store.upsert(ActiveSetpoint {
            param_id: "buffer.0.request_flow_temp".into(),
            write_group_id: Some("buffer.request_temps".into()),
            display_value: 35.0,
            group_values: Some(vec![GroupValue {
                param_id: "buffer.0.request_flow_temp".into(),
                display_value: 35.0,
            }]),
            activated_at: "t1".into(),
            last_write_at: None,
            last_error: None,
        });
        assert_eq!(store.len(), 2);
        assert!(store.remove("hk.1.flow_setpoint"));
        assert!(store.remove("buffer.request_temps"));
        assert!(store.is_empty());
    }

    #[test]
    fn json_roundtrip_model() {
        let mut store = ActiveSetpointStore::with_persist_path("/tmp/active_setpoints.json");
        store.upsert(ActiveSetpoint {
            param_id: "hk.1.flow_setpoint".into(),
            write_group_id: None,
            display_value: 40.0,
            group_values: None,
            activated_at: wall_now_iso8601(),
            last_write_at: None,
            last_error: None,
        });
        let json = store.serialize_json().unwrap();
        let mut other = ActiveSetpointStore::new();
        other.load_from_json(&json).unwrap();
        assert_eq!(other.entries(), store.entries());
        assert!(store.persist_path().is_some());
        assert!(store.save_stub().is_ok());
        assert!(other.load_stub().is_ok());
    }

    #[test]
    fn iso8601_epoch() {
        assert_eq!(format_unix_secs_iso8601(0), "1970-01-01T00:00:00Z");
        assert_eq!(format_unix_secs_iso8601(86_400), "1970-01-02T00:00:00Z");
    }
}
