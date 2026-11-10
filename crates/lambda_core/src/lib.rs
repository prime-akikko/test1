//! Application core: commands, events, poll / cyclic schedulers, stores.
//!
//! UI talks only to this crate. Modbus sockets stay behind `lambda_modbus`.
//!
//! ## Poll interval (M4)
//!
//! Production default is **[`defaults::POLL_INTERVAL_SECS`] = 5** seconds
//! ([`poll::DEFAULT_POLL_INTERVAL`]). Tests inject a short interval / [`FakeClock`]
//! and drive [`PollScheduler::tick`] — no real 5 s sleeps.
//!
//! ## Cyclic write interval (M5)
//!
//! Production default is **[`defaults::CYCLIC_WRITE_INTERVAL_SECS`] = 60** seconds
//! ([`cyclic::DEFAULT_CYCLIC_WRITE_INTERVAL`]). Confirmed user writes with
//! `persistence_class: cyclic` enter [`ActiveSetpointStore`]; [`CyclicWriteScheduler`]
//! rewrites them on the interval **without** a new confirmation. Tests use
//! [`FakeClock`] + short intervals. File persistence is stubbed until M8.
//!
//! GPUI arrives in M6+.

#![forbid(unsafe_code)]

mod active;
mod clock;
mod cyclic;
mod poll;
mod value;

pub use active::{
    format_unix_secs_iso8601, wall_now_iso8601, ActiveSetpoint, ActiveSetpointStore,
    ActiveSetpointsFile, GroupValue, ACTIVE_SETPOINTS_FILE_NAME, ACTIVE_SETPOINTS_SCHEMA_VERSION,
};
pub use clock::{Clock, FakeClock, SystemClock};
pub use cyclic::{CyclicWriteConfig, CyclicWriteScheduler, DEFAULT_CYCLIC_WRITE_INTERVAL};
pub use poll::{PollConfig, PollScheduler, DEFAULT_POLL_INTERVAL};
pub use value::{ParameterValue, ValueQuality};

pub use lambda_catalog;
pub use lambda_modbus::{
    ClientConfig, ModbusError, ModbusResult, ModbusTransport, QueuedTransport, TcpModbusClient,
    DEFAULT_PORT, DEFAULT_UNIT_ID, MAX_READ_QUANTITY, MAX_WRITE_QUANTITY,
};

/// Locked v1 defaults from product decisions.
pub mod defaults {
    pub const HOST: &str = "10.0.0.6";
    pub const PORT: u16 = super::DEFAULT_PORT;
    pub const UNIT_ID: u8 = super::DEFAULT_UNIT_ID;
    /// Production poll / read interval in seconds (architecture §10.1).
    pub const POLL_INTERVAL_SECS: u32 = 5;
    /// Production cyclic Active-Setpoint rewrite interval in seconds (architecture §10.2).
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

/// Why a poll cycle started (architecture §5.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PollTrigger {
    Timer,
    Manual,
    PostWrite,
}

/// Origin of a Modbus write (architecture §5.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WriteSource {
    /// Confirmed user write (first write / dialog).
    User,
    /// Cyclic rewrite of an Active Setpoint (no dialog).
    Cyclic,
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
    ConnectionChanged {
        status: ConnectionStatus,
    },
    PollStarted {
        trigger: PollTrigger,
    },
    PollCompleted {
        ok_count: u32,
        err_count: u32,
    },
    ParameterUpdated {
        param_id: String,
        raw: i64,
        display: f64,
        quality: ValueQuality,
    },
    ParameterError {
        param_id: String,
        message: String,
    },
    WriteSucceeded {
        param_id: Option<String>,
        write_group_id: Option<String>,
        source: WriteSource,
    },
    WriteFailed {
        param_id: Option<String>,
        write_group_id: Option<String>,
        message: String,
        source: WriteSource,
    },
    WriteRejected {
        reason: String,
    },
    ActiveSetpointsChanged,
    CyclicWriteStarted {
        count: u32,
    },
    CyclicWriteCompleted {
        ok_count: u32,
        err_count: u32,
    },
}

/// Lightweight handle for shared helpers (write confirmation, status defaults).
///
/// Live polling: [`PollScheduler`] (M4). Cyclic writes: [`CyclicWriteScheduler`] (M5).
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

/// Build a catalog-seeded mock transport when the `mock` feature is enabled.
#[cfg(feature = "mock")]
pub fn new_mock_transport(unit_id: u8) -> lambda_mock::MockSlave {
    lambda_mock::MockSlave::from_catalog(unit_id, lambda_catalog::Catalog::official())
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
        assert_eq!(DEFAULT_POLL_INTERVAL.as_secs(), 5);
        assert_eq!(DEFAULT_CYCLIC_WRITE_INTERVAL.as_secs(), 60);
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
