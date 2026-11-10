//! Production Modbus TCP client (FC 0x03 / 0x10). Host/port come from config — never hardcoded.

use std::io::{Read, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::time::Instant;

use crate::framing::{
    build_read_holding, build_write_multiple, parse_read_holding_response,
    parse_write_multiple_response, validate_read_count, validate_write_count,
};
use crate::{ClientConfig, ModbusError, ModbusResult, ModbusTransport};

/// TCP Modbus client. Target address is supplied only via [`ModbusTransport::connect`].
#[derive(Debug)]
pub struct TcpModbusClient {
    config: ClientConfig,
    stream: Option<TcpStream>,
    transaction_id: u16,
}

impl TcpModbusClient {
    pub fn new(config: ClientConfig) -> Self {
        Self {
            config,
            stream: None,
            transaction_id: 0,
        }
    }

    pub fn config(&self) -> &ClientConfig {
        &self.config
    }

    fn next_tid(&mut self) -> u16 {
        self.transaction_id = self.transaction_id.wrapping_add(1);
        if self.transaction_id == 0 {
            self.transaction_id = 1;
        }
        self.transaction_id
    }

    fn stream_mut(&mut self) -> ModbusResult<&mut TcpStream> {
        self.stream.as_mut().ok_or(ModbusError::NotConnected)
    }

    fn map_io(err: std::io::Error) -> ModbusError {
        match err.kind() {
            std::io::ErrorKind::TimedOut | std::io::ErrorKind::WouldBlock => ModbusError::Timeout,
            std::io::ErrorKind::ConnectionReset
            | std::io::ErrorKind::ConnectionAborted
            | std::io::ErrorKind::BrokenPipe
            | std::io::ErrorKind::UnexpectedEof => ModbusError::ConnectionClosed,
            std::io::ErrorKind::NotConnected => ModbusError::NotConnected,
            _ => ModbusError::Other(err.to_string()),
        }
    }

    fn transact(&mut self, request: &[u8]) -> ModbusResult<Vec<u8>> {
        {
            let stream = self.stream_mut()?;
            stream.write_all(request).map_err(Self::map_io)?;
            stream.flush().map_err(Self::map_io)?;
        }
        // Read MBAP (6) + at least unit+fc; then remainder from Length field.
        let mut header = [0u8; 7];
        {
            let stream = self.stream_mut()?;
            stream.read_exact(&mut header).map_err(Self::map_io)?;
        }
        let length = u16::from_be_bytes([header[4], header[5]]) as usize;
        if length < 2 {
            return Err(ModbusError::Other("Ungültige MBAP-Länge".into()));
        }
        // Length covers unit_id + PDU; we already have unit_id in header[6].
        let remaining = length - 1;
        let mut pdu = vec![0u8; remaining];
        {
            let stream = self.stream_mut()?;
            stream.read_exact(&mut pdu).map_err(Self::map_io)?;
        }
        let mut full = Vec::with_capacity(7 + remaining);
        full.extend_from_slice(&header);
        full.extend_from_slice(&pdu);
        Ok(full)
    }
}

impl Default for TcpModbusClient {
    fn default() -> Self {
        Self::new(ClientConfig::default())
    }
}

impl ModbusTransport for TcpModbusClient {
    fn connect(&mut self, host: &str, port: u16) -> ModbusResult<()> {
        self.disconnect();
        let addr = format!("{host}:{port}");
        let mut last_err = None;
        let deadline = Instant::now() + self.config.connect_timeout;
        let addrs = addr
            .to_socket_addrs()
            .map_err(|e| ModbusError::NetworkUnreachable {
                message: e.to_string(),
            })?;
        for socket_addr in addrs {
            let remain = deadline.saturating_duration_since(Instant::now());
            if remain.is_zero() {
                break;
            }
            match TcpStream::connect_timeout(&socket_addr, remain) {
                Ok(stream) => {
                    stream
                        .set_read_timeout(Some(self.config.request_timeout))
                        .map_err(|e| ModbusError::Other(e.to_string()))?;
                    stream
                        .set_write_timeout(Some(self.config.request_timeout))
                        .map_err(|e| ModbusError::Other(e.to_string()))?;
                    stream
                        .set_nodelay(true)
                        .map_err(|e| ModbusError::Other(e.to_string()))?;
                    self.stream = Some(stream);
                    return Ok(());
                }
                Err(e) => last_err = Some(e),
            }
        }
        Err(ModbusError::NetworkUnreachable {
            message: last_err
                .map(|e| e.to_string())
                .unwrap_or_else(|| format!("Ziel nicht erreichbar: {addr}")),
        })
    }

    fn disconnect(&mut self) {
        self.stream = None;
    }

    fn is_connected(&self) -> bool {
        self.stream.is_some()
    }

    fn read_holding(&mut self, unit: u8, addr: u16, count: u16) -> ModbusResult<Vec<u16>> {
        validate_read_count(count)?;
        if self.stream.is_none() {
            return Err(ModbusError::NotConnected);
        }
        let tid = self.next_tid();
        let req = build_read_holding(tid, unit, addr, count)?;
        let raw = self.transact(&req)?;
        parse_read_holding_response(&raw, tid, unit, count)
    }

    fn write_multiple(&mut self, unit: u8, addr: u16, words: &[u16]) -> ModbusResult<()> {
        validate_write_count(words.len())?;
        if self.stream.is_none() {
            return Err(ModbusError::NotConnected);
        }
        let tid = self.next_tid();
        let qty = words.len() as u16;
        let req = build_write_multiple(tid, unit, addr, words)?;
        let raw = self.transact(&req)?;
        parse_write_multiple_response(&raw, tid, unit, addr, qty)
    }
}
