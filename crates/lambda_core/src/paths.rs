//! App data directory resolution (architecture §4).
//!
//! Windows (v1): `%APPDATA%\LambdaModbusGui\`
//! Linux: `$XDG_CONFIG_HOME/LambdaModbusGui` or `~/.config/LambdaModbusGui`
//! macOS: `~/Library/Application Support/LambdaModbusGui`
//!
//! Override for tests / CI: env `LAMBDA_MODBUS_GUI_DATA`.

use std::env;
use std::path::PathBuf;

use crate::defaults;

/// Environment override used by Temp-Dir tests and agents.
pub const DATA_DIR_ENV: &str = "LAMBDA_MODBUS_GUI_DATA";

/// Resolve the application config directory (creates nothing — callers mkdir).
pub fn app_data_dir() -> Result<PathBuf, String> {
    if let Ok(custom) = env::var(DATA_DIR_ENV) {
        let trimmed = custom.trim();
        if !trimmed.is_empty() {
            return Ok(PathBuf::from(trimmed));
        }
    }

    #[cfg(windows)]
    {
        let appdata = env::var("APPDATA")
            .map_err(|_| "APPDATA nicht gesetzt — Windows-Config-Pfad unbekannt".to_string())?;
        return Ok(PathBuf::from(appdata).join(defaults::APP_DIR_NAME));
    }

    #[cfg(target_os = "macos")]
    {
        let home = env::var("HOME").map_err(|_| "HOME nicht gesetzt".to_string())?;
        return Ok(PathBuf::from(home)
            .join("Library")
            .join("Application Support")
            .join(defaults::APP_DIR_NAME));
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    {
        if let Ok(xdg) = env::var("XDG_CONFIG_HOME") {
            let trimmed = xdg.trim();
            if !trimmed.is_empty() {
                return Ok(PathBuf::from(trimmed).join(defaults::APP_DIR_NAME));
            }
        }
        let home = env::var("HOME").map_err(|_| "HOME nicht gesetzt".to_string())?;
        return Ok(PathBuf::from(home)
            .join(".config")
            .join(defaults::APP_DIR_NAME));
    }

    #[cfg(not(any(windows, unix)))]
    {
        Err("Kein bekannter Config-Pfad für diese Plattform".into())
    }
}

/// Ensure the app data directory exists.
pub fn ensure_app_data_dir() -> Result<PathBuf, String> {
    let dir = app_data_dir()?;
    std::fs::create_dir_all(&dir)
        .map_err(|e| format!("Config-Verzeichnis konnte nicht angelegt werden: {e}"))?;
    Ok(dir)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn env_override_wins() {
        let _g = ENV_LOCK.lock().unwrap();
        let tmp = std::env::temp_dir().join(format!(
            "lambda-modbus-gui-path-test-{}",
            std::process::id()
        ));
        env::set_var(DATA_DIR_ENV, &tmp);
        let got = app_data_dir().unwrap();
        env::remove_var(DATA_DIR_ENV);
        assert_eq!(got, tmp);
    }
}
