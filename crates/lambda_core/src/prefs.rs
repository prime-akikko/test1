//! `prefs.json` — UI preferences (architecture §4.2).

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::persist_io::{atomic_write_json, load_json_file, LoadOutcome};

pub const PREFS_SCHEMA_VERSION: u32 = 1;
pub const PREFS_FILE_NAME: &str = "prefs.json";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PrefsFile {
    pub schema_version: u32,
    pub selected_group: Option<String>,
    pub filter_query: String,
    pub show_raw_values: bool,
    pub show_read_only: bool,
    pub window_width: Option<u32>,
    pub window_height: Option<u32>,
    pub collapsed_groups: Vec<String>,
    pub show_active_setpoints_panel: bool,
}

impl Default for PrefsFile {
    fn default() -> Self {
        Self {
            schema_version: PREFS_SCHEMA_VERSION,
            selected_group: None,
            filter_query: String::new(),
            show_raw_values: true,
            show_read_only: true,
            window_width: None,
            window_height: None,
            collapsed_groups: Vec::new(),
            show_active_setpoints_panel: true,
        }
    }
}

impl PrefsFile {
    pub fn validate(&self) -> Result<(), String> {
        if self.schema_version == 0 {
            return Err("Ungültige schema_version: 0".into());
        }
        Ok(())
    }
}

pub fn load_prefs(dir: &Path) -> (PrefsFile, Option<String>) {
    let path = dir.join(PREFS_FILE_NAME);
    match load_json_file::<PrefsFile>(&path) {
        LoadOutcome::Missing => (PrefsFile::default(), None),
        LoadOutcome::Loaded(prefs) => match prefs.validate() {
            Ok(()) => (prefs, None),
            Err(msg) => (
                PrefsFile::default(),
                Some(format!("prefs.json ungültig ({msg}) — Defaults")),
            ),
        },
        LoadOutcome::Fallback { message } => (PrefsFile::default(), Some(message)),
    }
}

pub fn save_prefs(dir: &Path, prefs: &PrefsFile) -> Result<PathBuf, String> {
    prefs.validate()?;
    let path = dir.join(PREFS_FILE_NAME);
    atomic_write_json(&path, prefs)?;
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
        let dir = std::env::temp_dir().join(format!("lambda-prefs-{nanos}"));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn roundtrip_temp_dir() {
        let dir = temp_dir();
        let mut prefs = PrefsFile::default();
        prefs.selected_group = Some("Heizkreis 2".into());
        prefs.filter_query = "vorlauf".into();
        save_prefs(&dir, &prefs).unwrap();
        let (loaded, warn) = load_prefs(&dir);
        assert!(warn.is_none());
        assert_eq!(loaded.selected_group.as_deref(), Some("Heizkreis 2"));
        assert_eq!(loaded.filter_query, "vorlauf");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn corrupt_falls_back() {
        let dir = temp_dir();
        fs::write(dir.join(PREFS_FILE_NAME), "{bad").unwrap();
        let (prefs, warn) = load_prefs(&dir);
        assert_eq!(prefs, PrefsFile::default());
        assert!(warn.is_some());
        let _ = fs::remove_dir_all(&dir);
    }
}
