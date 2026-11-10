//! Atomic JSON file helpers (temp + rename).

use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};

use serde::de::DeserializeOwned;
use serde::Serialize;

/// Result of attempting to load a JSON document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LoadOutcome<T> {
    /// File missing → caller should use defaults.
    Missing,
    /// Parsed successfully.
    Loaded(T),
    /// Corrupt / invalid schema — defaults should be used; message for UI.
    Fallback { message: String },
}

/// Atomically write pretty JSON: `path.tmp` → fsync → rename over `path`.
pub fn atomic_write_json<T: Serialize>(path: &Path, value: &T) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| format!("Verzeichnis anlegen fehlgeschlagen: {e}"))?;
    }
    let tmp = temp_path_for(path);
    let json = serde_json::to_string_pretty(value)
        .map_err(|e| format!("JSON-Serialisierung fehlgeschlagen: {e}"))?;
    {
        let mut file =
            File::create(&tmp).map_err(|e| format!("Temp-Datei schreiben fehlgeschlagen: {e}"))?;
        file.write_all(json.as_bytes())
            .map_err(|e| format!("Temp-Datei schreiben fehlgeschlagen: {e}"))?;
        file.write_all(b"\n")
            .map_err(|e| format!("Temp-Datei schreiben fehlgeschlagen: {e}"))?;
        file.sync_all()
            .map_err(|e| format!("Temp-Datei sync fehlgeschlagen: {e}"))?;
    }
    fs::rename(&tmp, path).map_err(|e| {
        let _ = fs::remove_file(&tmp);
        format!("Atomares Umbenennen fehlgeschlagen: {e}")
    })?;
    Ok(())
}

/// Load JSON from disk. Missing → [`LoadOutcome::Missing`]. Corrupt → Fallback.
pub fn load_json_file<T: DeserializeOwned>(path: &Path) -> LoadOutcome<T> {
    if !path.exists() {
        return LoadOutcome::Missing;
    }
    match fs::read_to_string(path) {
        Ok(text) => match serde_json::from_str::<T>(&text) {
            Ok(v) => LoadOutcome::Loaded(v),
            Err(e) => LoadOutcome::Fallback {
                message: format!(
                    "Datei {} ist beschädigt ({e}) — Defaults werden verwendet",
                    path.display()
                ),
            },
        },
        Err(e) => LoadOutcome::Fallback {
            message: format!("Datei {} konnte nicht gelesen werden: {e}", path.display()),
        },
    }
}

fn temp_path_for(path: &Path) -> PathBuf {
    let file_name = path
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("data.json");
    path.with_file_name(format!(".{file_name}.{}.tmp", std::process::id()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::{Deserialize, Serialize};
    use std::time::{SystemTime, UNIX_EPOCH};

    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
    struct Doc {
        schema_version: u32,
        value: String,
    }

    fn tmp_file(name: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("lambda-persist-{name}-{nanos}.json"))
    }

    #[test]
    fn atomic_roundtrip() {
        let path = tmp_file("roundtrip");
        let doc = Doc {
            schema_version: 1,
            value: "hallo".into(),
        };
        atomic_write_json(&path, &doc).unwrap();
        match load_json_file::<Doc>(&path) {
            LoadOutcome::Loaded(got) => assert_eq!(got, doc),
            other => panic!("unexpected {other:?}"),
        }
        let _ = fs::remove_file(&path);
    }

    #[test]
    fn missing_file() {
        let path = tmp_file("missing");
        let _ = fs::remove_file(&path);
        assert_eq!(load_json_file::<Doc>(&path), LoadOutcome::Missing);
    }

    #[test]
    fn corrupt_json_fallback() {
        let path = tmp_file("corrupt");
        fs::write(&path, "{ not json").unwrap();
        match load_json_file::<Doc>(&path) {
            LoadOutcome::Fallback { message } => {
                assert!(message.contains("beschädigt") || message.contains("Defaults"));
            }
            other => panic!("unexpected {other:?}"),
        }
        let _ = fs::remove_file(&path);
    }
}
