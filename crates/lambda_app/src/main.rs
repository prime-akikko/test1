//! Lambda Modbus GPUI binary entry (shell arrives in M6).
//!
//! Default Cloud/CI build uses the `mock` feature and does not link GPUI.
//! Windows GUI: `cargo run -p lambda_app --features gui`.

#![forbid(unsafe_code)]

use lambda_core::{defaults, mock_backend_enabled, CoreRuntime};

fn main() {
    let runtime = CoreRuntime::new();
    println!("LambdaModbusGui M1 scaffold");
    println!(
        "Defaults: {}:{} unit {} | poll {}s | cyclic {}s",
        defaults::HOST,
        defaults::PORT,
        defaults::UNIT_ID,
        defaults::POLL_INTERVAL_SECS,
        defaults::CYCLIC_WRITE_INTERVAL_SECS
    );
    println!("Mock-Backend aktiv: {}", mock_backend_enabled());
    println!("Verbindungsstatus: {:?}", runtime.status());

    #[cfg(feature = "gui")]
    {
        // Force-link the pinned crates.io gpui; window bootstrap is M6.
        let _ = std::any::type_name::<gpui::App>();
        println!("GPUI-Feature aktiv (Pin crates.io 0.2.2) — UI-Shell folgt in M6");
    }

    #[cfg(not(feature = "gui"))]
    {
        println!("GPUI nicht gelinkt (Feature `gui` aus). Für Windows-GUI: --features gui");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn smoke_runtime() {
        let rt = CoreRuntime::new();
        assert!(matches!(
            rt.status(),
            lambda_core::ConnectionStatus::Disconnected
        ));
    }
}
