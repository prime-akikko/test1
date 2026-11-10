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
    use lambda_app::{session::AppSession, state::ShellState, strings};

    println!("{}", strings::WINDOW_TITLE);
    println!("{}", strings::APP_SUBHEADING);

    let mut state = ShellState::new(cfg!(feature = "mock"));
    let mut session = AppSession::from_shell_defaults();

    println!(
        "Defaults: {}:{} unit {}",
        state.connection.host, state.connection.port, state.connection.unit_id
    );
    println!("Backend: {}", state.backend_label());
    println!("Gruppen: {}", state.groups.join(", "));

    // Exercise mock connect → poll → disconnect so CI proves wiring without GPUI.
    #[cfg(feature = "mock")]
    {
        session.connect(&mut state);
        println!("Nach Verbinden: {}", state.status_label());
        if let Some(poll) = &state.last_poll {
            println!(
                "Erster Poll: {} ok / {} Fehler",
                poll.ok_count, poll.err_count
            );
        }
        session.disconnect(&mut state);
        println!("Nach Trennen: {}", state.status_label());
    }

    #[cfg(not(feature = "mock"))]
    {
        let _ = (&mut session, &mut state);
        println!("Mock-Feature aus — kein automatischer Connect (live TCP).");
        println!("GPUI nicht gelinkt. Für Windows-GUI: --features gui");
    }

    println!("M6-Shell-State bereit (GUI-Feature aus).");
}
