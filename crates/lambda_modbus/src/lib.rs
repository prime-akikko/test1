//! Modbus TCP transport trait, framing, serialized queue, and production client.
//!
//! Holding registers only: read FC `0x03`, write FC `0x10`.
//! All requests share one serialized queue (poll must not race cyclic writes).
//!
//! **Max quantity:** reads ≤ [`MAX_READ_QUANTITY`] (125), writes ≤ [`MAX_WRITE_QUANTITY`] (123).
//! Catalog read-block planner must stay within these limits; the client rejects larger counts.
//!
//! Target host/port are never hardcoded — only [`ClientConfig`] timeouts / unit defaults
//! and the address passed to [`ModbusTransport::connect`].

#![forbid(unsafe_code)]

mod framing;
mod queue;
mod tcp;

pub use framing::{
    build_read_holding, build_write_multiple, parse_read_holding_response,
    parse_write_multiple_response, validate_read_count, validate_write_count, FC_READ_HOLDING,
    FC_WRITE_MULTIPLE,
};
pub use queue::{ConcurrencyProbe, QueuedTransport};
pub use tcp::TcpModbusClient;

use std::fmt;
use std::time::Duration;

/// Default Modbus Unit / Slave ID (locked v1).
pub const DEFAULT_UNIT_ID: u8 = 1;

/// Default TCP port (locked v1). Not a default host — host comes from config/UI.
pub const DEFAULT_PORT: u16 = 502;

/// Practical Modbus max quantity for a single FC 0x03 request.
pub const MAX_READ_QUANTITY: u16 = 125;

/// Practical Modbus max quantity for a single FC 0x10 request.
pub const MAX_WRITE_QUANTITY: u16 = 123;

/// Default connect timeout (matches architecture `connect_timeout_ms`).
pub const DEFAULT_CONNECT_TIMEOUT_MS: u64 = 3000;

/// Default per-request timeout (matches architecture `request_timeout_ms`).
pub const DEFAULT_REQUEST_TIMEOUT_MS: u64 = 3000;

/// Transport-level failures mapped for core/UI (architecture §7).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ModbusError {
    NetworkUnreachable { message: String },
    Timeout,
    ConnectionClosed,
    NotConnected,
    ModbusException { code: u8 },
    InvalidArgument(&'static str),
    Other(String),
}

impl fmt::Display for ModbusError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NetworkUnreachable { message } => {
                write!(f, "Netzwerk nicht erreichbar: {message}")
            }
            Self::Timeout => write!(f, "Modbus-Timeout"),
            Self::ConnectionClosed => write!(f, "Verbindung geschlossen"),
            Self::NotConnected => write!(f, "Nicht verbunden"),
            Self::ModbusException { code } => write!(f, "Modbus-Exception 0x{code:02X}"),
            Self::InvalidArgument(msg) => write!(f, "Ungültiges Argument: {msg}"),
            Self::Other(msg) => write!(f, "{msg}"),
        }
    }
}

impl std::error::Error for ModbusError {}

pub type ModbusResult<T> = Result<T, ModbusError>;

/// Timeouts and default unit — **no host/IP** (avoids hardcoding `10.0.0.6`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClientConfig {
    /// Preferred unit id for callers that do not override per-request.
    pub unit_id: u8,
    pub connect_timeout: Duration,
    pub request_timeout: Duration,
}

impl Default for ClientConfig {
    fn default() -> Self {
        Self {
            unit_id: DEFAULT_UNIT_ID,
            connect_timeout: Duration::from_millis(DEFAULT_CONNECT_TIMEOUT_MS),
            request_timeout: Duration::from_millis(DEFAULT_REQUEST_TIMEOUT_MS),
        }
    }
}

impl ClientConfig {
    pub fn with_timeouts(connect_timeout: Duration, request_timeout: Duration) -> Self {
        Self {
            unit_id: DEFAULT_UNIT_ID,
            connect_timeout,
            request_timeout,
        }
    }
}

/// Shared transport contract for production TCP and `lambda_mock`.
pub trait ModbusTransport {
    fn connect(&mut self, host: &str, port: u16) -> ModbusResult<()>;
    fn disconnect(&mut self);
    fn is_connected(&self) -> bool;

    /// FC 0x03 — read holding registers.
    fn read_holding(&mut self, unit: u8, addr: u16, count: u16) -> ModbusResult<Vec<u16>>;

    /// FC 0x10 — write multiple holding registers (also used for single-register writes).
    fn write_multiple(&mut self, unit: u8, addr: u16, words: &[u16]) -> ModbusResult<()>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_match_locked_product() {
        assert_eq!(DEFAULT_UNIT_ID, 1);
        assert_eq!(DEFAULT_PORT, 502);
        assert_eq!(MAX_READ_QUANTITY, 125);
        let cfg = ClientConfig::default();
        assert_eq!(cfg.unit_id, 1);
        assert_eq!(cfg.connect_timeout.as_millis(), 3000);
        assert_eq!(cfg.request_timeout.as_millis(), 3000);
    }

    #[test]
    fn tcp_client_starts_disconnected_without_host() {
        let client = TcpModbusClient::new(ClientConfig::default());
        assert!(!client.is_connected());
        // Ensure source does not bake in plant IP — config has no host field.
        let _ = client.config().unit_id;
    }

    #[test]
    fn tcp_connect_to_closed_port_is_unreachable() {
        let mut client = TcpModbusClient::new(ClientConfig::with_timeouts(
            Duration::from_millis(200),
            Duration::from_millis(200),
        ));
        // High port unlikely to accept; must not panic or hang beyond timeout.
        let err = client.connect("127.0.0.1", 1).unwrap_err();
        assert!(
            matches!(
                err,
                ModbusError::NetworkUnreachable { .. } | ModbusError::Timeout
            ),
            "unexpected error: {err:?}"
        );
        assert!(!client.is_connected());
    }
}
