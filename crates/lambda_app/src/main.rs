//! Lambda Modbus GPUI binary.
//!
//! Default Cloud/CI build: `mock` feature, no GPUI link.
//! Windows GUI: `cargo run -p lambda_app --features gui` (keep `mock` for
//! in-memory smoke, or `--no-default-features --features gui` for live TCP).

#![forbid(unsafe_code)]

fn main() {
    #[cfg(feature = "gui")]
    {
        lambda_app::ui::run_gui();
    }

    #[cfg(not(feature = "gui"))]
    {
        run_headless_smoke();
    }
}

/// Headless smoke path for Cloud/CI without native GUI libs.
#[cfg(not(feature = "gui"))]
fn run_headless_smoke() {
    use lambda_app::{persist::BootstrappedApp, strings};
    use std::time::{SystemTime, UNIX_EPOCH};

    println!("{}", strings::WINDOW_TITLE);
    println!("{}", strings::APP_SUBHEADING);

    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let dir = std::env::temp_dir().join(format!("lambda-modbus-gui-smoke-{nanos}"));
    let mut boot =
        BootstrappedApp::load_from(&dir, cfg!(feature = "mock")).expect("Temp-Dir Persistenz");

    println!(
        "Defaults/Config: {}:{} unit {} (dir {})",
        boot.state.connection.host,
        boot.state.connection.port,
        boot.state.connection.unit_id,
        boot.data_dir.display()
    );
    println!("Backend: {}", boot.state.backend_label());
    println!("Gruppen: {}", boot.state.groups.join(", "));

    #[cfg(feature = "mock")]
    {
        boot.session.connect(&mut boot.state);
        println!("Nach Verbinden: {}", boot.state.status_label());
        if let Some(poll) = &boot.state.last_poll {
            println!(
                "Erster Poll: {} ok / {} Fehler",
                poll.ok_count, poll.err_count
            );
        }
        let _ = lambda_app::persist::save_connection_settings(&boot.data_dir, &boot.state);
        boot.session.disconnect(&mut boot.state);
        println!("Nach Trennen: {}", boot.state.status_label());
        println!(
            "Persistenz: config.json vorhanden = {}",
            boot.data_dir.join("config.json").is_file()
        );
    }

    #[cfg(not(feature = "mock"))]
    {
        println!("Mock-Feature aus — kein automatischer Connect (live TCP).");
        println!("GPUI nicht gelinkt. Für Windows-GUI: --features gui");
    }

    let _ = std::fs::remove_dir_all(&dir);
    println!("M8-Persistenz-Smoke bereit (GUI-Feature aus).");
}
