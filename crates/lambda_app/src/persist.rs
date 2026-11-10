//! Load/save wiring between shell UI state and `lambda_core` file stores (M8).

use std::path::PathBuf;

use lambda_core::{
    save_config, save_prefs, ActiveSetpointStore, ConfigFile, LoadedStores, PrefsFile,
    ACTIVE_SETPOINTS_FILE_NAME,
};

use crate::session::AppSession;
use crate::state::ShellState;
use crate::strings;

/// Result of booting the app with persistence.
pub struct BootstrappedApp {
    pub state: ShellState,
    pub session: AppSession,
    pub data_dir: PathBuf,
}

impl BootstrappedApp {
    /// Load from platform app-data dir (or `LAMBDA_MODBUS_GUI_DATA`).
    pub fn load_default(mock_backend: bool) -> Self {
        match LoadedStores::load_default() {
            Ok(loaded) => Self::from_loaded(loaded, mock_backend),
            Err(err) => {
                let mut state = ShellState::new(mock_backend);
                state.status_message = format!(
                    "{}: {err} — starte mit Defaults",
                    strings::STATUS_ERROR_PREFIX
                );
                Self {
                    state,
                    session: AppSession::from_shell_defaults(),
                    data_dir: PathBuf::new(),
                }
            }
        }
    }

    /// Load from an explicit directory (Temp-Dir tests).
    pub fn load_from(dir: impl Into<PathBuf>, mock_backend: bool) -> Result<Self, String> {
        let loaded = LoadedStores::load_from(dir)?;
        Ok(Self::from_loaded(loaded, mock_backend))
    }

    fn from_loaded(loaded: LoadedStores, mock_backend: bool) -> Self {
        let mut state = ShellState::new(mock_backend);
        apply_config_to_state(&mut state, &loaded.config);
        apply_prefs_to_state(&mut state, &loaded.prefs);

        let mut session = AppSession::new(loaded.config.unit_id);
        session.install_active_store(loaded.active);
        state.sync_active_from(session.active_entries());

        let mut messages = loaded.warnings;
        if loaded.active_restored {
            messages.push(format!(
                "{} ({} Einträge) — Nachschreiben startet nach Verbinden",
                strings::MSG_ACTIVE_RESTORED,
                state.active_count
            ));
        }
        if !messages.is_empty() {
            let prefix = if state.status_message.is_empty() {
                String::new()
            } else {
                format!("{} · ", state.status_message)
            };
            state.status_message = format!("{prefix}{}", messages.join(" · "));
        }

        Self {
            state,
            session,
            data_dir: loaded.dir,
        }
    }
}

pub fn apply_config_to_state(state: &mut ShellState, cfg: &ConfigFile) {
    state.connection.host = cfg.host.clone();
    state.connection.port = cfg.port.to_string();
    state.connection.unit_id = cfg.unit_id.to_string();
}

pub fn config_from_state(state: &ShellState) -> Result<ConfigFile, String> {
    let mut cfg = ConfigFile::default();
    cfg.host = state.connection.host.trim().to_string();
    cfg.port = state.connection.parse_port().map_err(|e| e.to_string())?;
    cfg.unit_id = state
        .connection
        .parse_unit_id()
        .map_err(|e| e.to_string())?;
    Ok(cfg)
}

pub fn prefs_from_state(state: &ShellState) -> PrefsFile {
    PrefsFile {
        selected_group: Some(state.selected_group.clone()),
        filter_query: state.filter_query.clone(),
        ..PrefsFile::default()
    }
}

pub fn apply_prefs_to_state(state: &mut ShellState, prefs: &PrefsFile) {
    if let Some(group) = &prefs.selected_group {
        state.select_group(group.clone());
    }
    state.filter_query = prefs.filter_query.clone();
}

/// Persist connection settings (`SaveConnectionSettings`).
pub fn save_connection_settings(data_dir: &PathBuf, state: &ShellState) -> Result<(), String> {
    if data_dir.as_os_str().is_empty() {
        return Ok(());
    }
    let cfg = config_from_state(state)?;
    save_config(data_dir, &cfg).map(|_| ())
}

/// Persist UI prefs after navigation / filter changes.
pub fn save_ui_prefs(data_dir: &PathBuf, state: &ShellState) -> Result<(), String> {
    if data_dir.as_os_str().is_empty() {
        return Ok(());
    }
    save_prefs(data_dir, &prefs_from_state(state)).map(|_| ())
}

/// Ensure cyclic store writes under `data_dir/active_setpoints.json`.
pub fn active_path(data_dir: &PathBuf) -> PathBuf {
    data_dir.join(ACTIVE_SETPOINTS_FILE_NAME)
}

/// Attach persist path to an existing in-memory store (if path empty, leave unset).
pub fn ensure_active_persist_path(store: &mut ActiveSetpointStore, data_dir: &PathBuf) {
    if data_dir.as_os_str().is_empty() {
        return;
    }
    store.set_persist_path(active_path(data_dir));
}

#[cfg(test)]
mod tests {
    use super::*;
    use lambda_core::ActiveSetpoint;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_dir() -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("lambda-app-persist-{nanos}"));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn bootstrap_restores_active_and_config() {
        let dir = temp_dir();
        let mut boot = BootstrappedApp::load_from(&dir, true).unwrap();
        boot.state.connection.host = "10.1.2.3".into();
        boot.state.select_group("Heizkreis 2");
        save_connection_settings(&boot.data_dir, &boot.state).unwrap();
        save_ui_prefs(&boot.data_dir, &boot.state).unwrap();

        // Seed active via store API.
        let mut store = ActiveSetpointStore::with_persist_path(active_path(&dir));
        store.upsert(ActiveSetpoint {
            param_id: "hk.2.flow_setpoint".into(),
            write_group_id: None,
            display_value: 33.0,
            group_values: None,
            activated_at: "t".into(),
            last_write_at: None,
            last_error: None,
        });
        store.save().unwrap();

        let again = BootstrappedApp::load_from(&dir, true).unwrap();
        assert_eq!(again.state.connection.host, "10.1.2.3");
        assert_eq!(again.state.selected_group, "Heizkreis 2");
        assert_eq!(again.state.active_count, 1);
        assert!(
            again.state.status_message.contains("wiederhergestellt")
                || again.state.status_message.contains("Nachschreiben")
        );
        let _ = fs::remove_dir_all(&dir);
    }
}
