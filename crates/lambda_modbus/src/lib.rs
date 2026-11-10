//! Modbus TCP transport trait and error model (skeleton — implemented in M3).
//!
//! Holding registers only: read FC `0x03`, write FC `0x10`.
//! All requests share one serialized queue (poll must not race cyclic writes).

#![forbid(unsafe_code)]

use std::fmt;

/// Default Modbus Unit / Slave ID (locked v1).
pub const DEFAULT_UNIT_ID: u8 = 1;

/// Default TCP port.
pub const DEFAULT_PORT: u16 = 502;

/// Transport-level failures mapped for core/UI.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ModbusError {
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

/// Placeholder TCP client (no I/O until M3).
#[derive(Debug, Default)]
pub struct TcpModbusClient {
    connected: bool,
}

impl TcpModbusClient {
    pub fn new() -> Self {
        Self::default()
    }
}

impl ModbusTransport for TcpModbusClient {
    fn connect(&mut self, _host: &str, _port: u16) -> ModbusResult<()> {
        // Real TCP arrives in M3. Stub keeps the trait surface stable for M1.
        Err(ModbusError::Other(
            "TcpModbusClient noch nicht implementiert (M3)".into(),
        ))
    }

    fn disconnect(&mut self) {
        self.connected = false;
    }

    fn is_connected(&self) -> bool {
        self.connected
    }

    fn read_holding(&mut self, _unit: u8, _addr: u16, _count: u16) -> ModbusResult<Vec<u16>> {
        Err(ModbusError::NotConnected)
    }

    fn write_multiple(&mut self, _unit: u8, _addr: u16, _words: &[u16]) -> ModbusResult<()> {
        Err(ModbusError::NotConnected)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_match_locked_product() {
        assert_eq!(DEFAULT_UNIT_ID, 1);
        assert_eq!(DEFAULT_PORT, 502);
    }

    #[test]
    fn tcp_stub_starts_disconnected() {
        let client = TcpModbusClient::new();
        assert!(!client.is_connected());
    }
}
