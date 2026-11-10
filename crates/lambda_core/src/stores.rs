//! Facade: load/save all three persistence files under one app-data directory.

use std::path::PathBuf;

use crate::active::{ActiveSetpointStore, ACTIVE_SETPOINTS_FILE_NAME};
use crate::config::{load_config, save_config, ConfigFile};
use crate::paths::{ensure_app_data_dir, DATA_DIR_ENV};
use crate::prefs::{load_prefs, save_prefs, PrefsFile};

/// Bundle loaded at app start.
#[derive(Debug, Clone)]
pub struct LoadedStores {
    pub dir: PathBuf,
    pub config: ConfigFile,
    pub prefs: PrefsFile,
    pub active: ActiveSetpointStore,
    /// Non-fatal warnings (corrupt files, etc.) for the UI status line.
    pub warnings: Vec<String>,
    /// True when active setpoints were restored from disk (non-empty).
    pub active_restored: bool,
}

impl LoadedStores {
    /// Load from `dir` (must exist or be creatable).
    pub fn load_from(dir: impl Into<PathBuf>) -> Result<Self, String> {
        let dir = dir.into();
        std::fs::create_dir_all(&dir)
            .map_err(|e| format!("Config-Verzeichnis anlegen fehlgeschlagen: {e}"))?;

        let mut warnings = Vec::new();
        let (config, w) = load_config(&dir);
        if let Some(m) = w {
            warnings.push(m);
        }
        let (prefs, w) = load_prefs(&dir);
        if let Some(m) = w {
            warnings.push(m);
        }

        let active_path = dir.join(ACTIVE_SETPOINTS_FILE_NAME);
        let mut active = ActiveSetpointStore::with_persist_path(&active_path);
        if let Some(m) = active.load()? {
            warnings.push(m);
        }
        let active_restored = !active.is_empty();

        Ok(Self {
            dir,
            config,
            prefs,
            active,
            warnings,
            active_restored,
        })
    }

    /// Load from the platform app-data dir (honours [`DATA_DIR_ENV`]).
    pub fn load_default() -> Result<Self, String> {
        let dir = ensure_app_data_dir()?;
        Self::load_from(dir)
    }

    pub fn save_config(&self) -> Result<(), String> {
        save_config(&self.dir, &self.config).map(|_| ())
    }

    pub fn save_prefs(&self) -> Result<(), String> {
        save_prefs(&self.dir, &self.prefs).map(|_| ())
    }

    pub fn save_active(&self) -> Result<(), String> {
        self.active.save()
    }

    pub fn save_all(&self) -> Result<(), String> {
        self.save_config()?;
        self.save_prefs()?;
        self.save_active()?;
        Ok(())
    }

    pub fn data_dir_env_key() -> &'static str {
        DATA_DIR_ENV
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::active::ActiveSetpoint;
    use crate::paths::DATA_DIR_ENV;
    use std::fs;
    use std::sync::Mutex;
    use std::time::{SystemTime, UNIX_EPOCH};

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    fn temp_dir() -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("lambda-stores-{nanos}"));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn three_file_roundtrip() {
        let dir = temp_dir();
        let mut loaded = LoadedStores::load_from(&dir).unwrap();
        loaded.config.host = "10.0.0.99".into();
        loaded.prefs.selected_group = Some("Puffer".into());
        loaded.active.upsert(ActiveSetpoint {
            param_id: "hk.1.flow_setpoint".into(),
            write_group_id: None,
            display_value: 41.0,
            group_values: None,
            activated_at: "t".into(),
            last_write_at: None,
            last_error: None,
        });
        loaded.save_all().unwrap();

        let again = LoadedStores::load_from(&dir).unwrap();
        assert_eq!(again.config.host, "10.0.0.99");
        assert_eq!(again.prefs.selected_group.as_deref(), Some("Puffer"));
        assert_eq!(again.active.len(), 1);
        assert!(again.active_restored);
        assert!(dir.join("config.json").is_file());
        assert!(dir.join("prefs.json").is_file());
        assert!(dir.join("active_setpoints.json").is_file());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn env_override_load_default() {
        let _g = ENV_LOCK.lock().unwrap();
        let dir = temp_dir();
        std::env::set_var(DATA_DIR_ENV, &dir);
        let loaded = LoadedStores::load_default().unwrap();
        std::env::remove_var(DATA_DIR_ENV);
        assert_eq!(loaded.dir, dir);
        let _ = fs::remove_dir_all(&dir);
    }
}
