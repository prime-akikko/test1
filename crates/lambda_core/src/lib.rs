//! Application core: commands, events, schedulers, stores (skeleton — M4+).
//!
//! UI talks only to this crate. Modbus sockets stay behind `lambda_modbus`.

#![forbid(unsafe_code)]

pub use lambda_catalog;
pub use lambda_modbus::{
    ModbusError, ModbusResult, ModbusTransport, DEFAULT_PORT, DEFAULT_UNIT_ID,
};

/// Locked v1 defaults from product decisions.
pub mod defaults {
    pub const HOST: &str = "10.0.0.6";
    pub const PORT: u16 = super::DEFAULT_PORT;
    pub const UNIT_ID: u8 = super::DEFAULT_UNIT_ID;
    pub const POLL_INTERVAL_SECS: u32 = 5;
    pub const CYCLIC_WRITE_INTERVAL_SECS: u32 = 60;
    pub const CONNECT_TIMEOUT_MS: u32 = 3000;
    pub const REQUEST_TIMEOUT_MS: u32 = 3000;
    pub const APP_DIR_NAME: &str = "LambdaModbusGui";
}

/// Connection lifecycle as seen by the UI.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConnectionStatus {
    Disconnected,
    Connecting,
    Connected,
    Reconnecting,
    Error { message: String },
}

/// Commands from UI → core (Internal API surface).
#[derive(Debug, Clone, PartialEq)]
pub enum Command {
    Connect {
        host: String,
        port: u16,
        unit_id: u8,
    },
    Disconnect,
    SaveConnectionSettings {
        host: String,
        port: u16,
        unit_id: u8,
    },
    ReadAll,
    ReadGroup {
        group_id: String,
    },
    WriteRegister {
        param_id: String,
        display_value: f64,
        confirmed: bool,
    },
    WriteRegisterGroup {
        write_group_id: String,
        values: Vec<(String, f64)>,
        confirmed: bool,
    },
    ClearActiveSetpoint {
        param_id: String,
    },
    ClearAllActiveSetpoints {
        confirmed: bool,
    },
}

/// Events from core → UI.
#[derive(Debug, Clone, PartialEq)]
pub enum Event {
    ConnectionChanged { status: ConnectionStatus },
    PollStarted,
    PollCompleted { ok_count: u32, err_count: u32 },
    WriteRejected { reason: String },
    ActiveSetpointsChanged,
}

/// Placeholder runtime handle (schedulers arrive in M4/M5).
#[derive(Debug)]
pub struct CoreRuntime {
    status: ConnectionStatus,
    #[cfg(feature = "mock")]
    _mock_marker: (),
}

impl CoreRuntime {
    pub fn new() -> Self {
        Self {
            status: ConnectionStatus::Disconnected,
            #[cfg(feature = "mock")]
            _mock_marker: (),
        }
    }

    pub fn status(&self) -> &ConnectionStatus {
        &self.status
    }

    /// Reject first writes unless `confirmed` (safety invariant for later M7).
    pub fn validate_write_confirmed(confirmed: bool) -> Result<(), Event> {
        if confirmed {
            Ok(())
        } else {
            Err(Event::WriteRejected {
                reason: "Schreibvorgang nicht bestätigt".into(),
            })
        }
    }
}

impl Default for CoreRuntime {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(feature = "mock")]
pub fn mock_backend_enabled() -> bool {
    true
}

#[cfg(not(feature = "mock"))]
pub fn mock_backend_enabled() -> bool {
    false
}

/// Build a disconnected mock transport when the `mock` feature is enabled.
#[cfg(feature = "mock")]
pub fn new_mock_transport(unit_id: u8) -> lambda_mock::MockSlave {
    lambda_mock::MockSlave::new(unit_id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn locked_timer_defaults() {
        assert_eq!(defaults::POLL_INTERVAL_SECS, 5);
        assert_eq!(defaults::CYCLIC_WRITE_INTERVAL_SECS, 60);
        assert_eq!(defaults::HOST, "10.0.0.6");
        assert_eq!(defaults::UNIT_ID, 1);
    }

    #[test]
    fn write_requires_confirmation() {
        assert!(CoreRuntime::validate_write_confirmed(true).is_ok());
        assert!(matches!(
            CoreRuntime::validate_write_confirmed(false),
            Err(Event::WriteRejected { .. })
        ));
    }

    #[test]
    fn runtime_starts_disconnected() {
        let rt = CoreRuntime::new();
        assert_eq!(rt.status(), &ConnectionStatus::Disconnected);
    }
}
