//! In-memory Modbus holding-register slave for mock-first tests.
//!
//! Implements the same [`ModbusTransport`] trait as the production TCP client.
//! Seed from the official catalog; inject timeouts, exceptions, and partial failures.
//! Never targets live `10.0.0.6` from CI/Cloud.

#![forbid(unsafe_code)]

mod tcp_server;

pub use tcp_server::{spawn_tcp_mock_server, TcpMockServer};

use std::collections::{BTreeMap, BTreeSet};
use std::time::Duration;

use lambda_catalog::{assemble_int32, split_int32, Catalog};
use lambda_modbus::{
    validate_read_count, validate_write_count, ModbusError, ModbusResult, ModbusTransport,
};

/// One-shot or sticky fault injected into the next matching operation.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum InjectedFault {
    #[default]
    None,
    /// Simulate request timeout (no successful I/O).
    Timeout,
    /// Drop the logical connection.
    ConnectionClosed,
    /// Return a Modbus exception code (e.g. 0x02 illegal address).
    Exception(u8),
    /// Fail reads/writes that overlap `[start, start+count)`.
    AddressRange { start: u16, count: u16, code: u8 },
}

/// In-memory Modbus slave keyed by PDU holding-register address.
#[derive(Debug)]
pub struct MockSlave {
    connected: bool,
    unit_id: u8,
    registers: BTreeMap<u16, u16>,
    /// Number of successful FC 0x10 calls (for cyclic-write assertions later).
    pub write_count: u64,
    /// Number of successful FC 0x03 calls.
    pub read_count: u64,
    fault: InjectedFault,
    /// Consume fault after one I/O attempt when true.
    fault_once: bool,
    /// Artificial delay applied inside each I/O (for queue/timing tests).
    io_delay: Duration,
}

impl MockSlave {
    pub fn new(unit_id: u8) -> Self {
        Self {
            connected: false,
            unit_id,
            registers: BTreeMap::new(),
            write_count: 0,
            read_count: 0,
            fault: InjectedFault::None,
            fault_once: false,
            io_delay: Duration::ZERO,
        }
    }

    /// Seed every catalog parameter address with zero (INT32 → two words).
    pub fn from_catalog(unit_id: u8, catalog: &Catalog) -> Self {
        let mut slave = Self::new(unit_id);
        slave.seed_catalog_defaults(catalog);
        slave
    }

    /// Official catalog, Unit ID 1.
    pub fn from_official_catalog() -> Self {
        Self::from_catalog(1, Catalog::official())
    }

    pub fn unit_id(&self) -> u8 {
        self.unit_id
    }

    pub fn seed(&mut self, addr: u16, value: u16) {
        self.registers.insert(addr, value);
    }

    pub fn seed_i16(&mut self, addr: u16, value: i16) {
        self.registers.insert(addr, value as u16);
    }

    pub fn seed_i32(&mut self, addr: u16, value: i32) {
        let (high, low) = split_int32(value);
        self.registers.insert(addr, high);
        self.registers.insert(addr.saturating_add(1), low);
    }

    pub fn get(&self, addr: u16) -> Option<u16> {
        self.registers.get(&addr).copied()
    }

    pub fn get_i32(&self, addr: u16) -> Option<i32> {
        let high = self.registers.get(&addr).copied()?;
        let low = self.registers.get(&(addr + 1)).copied()?;
        Some(assemble_int32(high, low))
    }

    /// Initialize all catalog addresses to 0.
    pub fn seed_catalog_defaults(&mut self, catalog: &Catalog) {
        let mut covered = BTreeSet::new();
        for p in &catalog.parameters {
            for offset in 0..p.register_count {
                let addr = p.address.saturating_add(u16::from(offset));
                if covered.insert(addr) {
                    self.registers.entry(addr).or_insert(0);
                }
            }
        }
    }

    pub fn inject_fault(&mut self, fault: InjectedFault) {
        self.fault = fault;
        self.fault_once = false;
    }

    /// Inject a fault that clears itself after the next I/O attempt.
    pub fn inject_fault_once(&mut self, fault: InjectedFault) {
        self.fault = fault;
        self.fault_once = true;
    }

    pub fn clear_fault(&mut self) {
        self.fault = InjectedFault::None;
        self.fault_once = false;
    }

    pub fn set_io_delay(&mut self, delay: Duration) {
        self.io_delay = delay;
    }

    pub fn register_count_seeded(&self) -> usize {
        self.registers.len()
    }

    fn apply_delay(&self) {
        if !self.io_delay.is_zero() {
            std::thread::sleep(self.io_delay);
        }
    }

    fn take_fault(&mut self) -> InjectedFault {
        let fault = self.fault.clone();
        if self.fault_once {
            self.fault = InjectedFault::None;
            self.fault_once = false;
        }
        fault
    }

    fn check_connected_and_unit(&self, unit: u8) -> ModbusResult<()> {
        if !self.connected {
            return Err(ModbusError::NotConnected);
        }
        if unit != self.unit_id {
            return Err(ModbusError::ModbusException { code: 0x0B });
        }
        Ok(())
    }

    fn overlaps_fail_range(start: u16, count: u16, range_start: u16, range_count: u16) -> bool {
        let end = start.saturating_add(count);
        let range_end = range_start.saturating_add(range_count);
        start < range_end && range_start < end
    }

    fn eval_fault(&mut self, start: u16, count: u16) -> ModbusResult<()> {
        // Peek first for address-range sticky faults so non-overlapping ops keep the fault.
        if let InjectedFault::AddressRange {
            start: rs,
            count: rc,
            code,
        } = self.fault.clone()
        {
            if Self::overlaps_fail_range(start, count, rs, rc) {
                let _ = self.take_fault();
                return Err(ModbusError::ModbusException { code });
            }
            // Non-overlapping: leave sticky fault in place; consume only if once.
            if self.fault_once {
                let _ = self.take_fault();
            }
            return Ok(());
        }

        match self.take_fault() {
            InjectedFault::None => Ok(()),
            InjectedFault::Timeout => Err(ModbusError::Timeout),
            InjectedFault::ConnectionClosed => {
                self.connected = false;
                Err(ModbusError::ConnectionClosed)
            }
            InjectedFault::Exception(code) => Err(ModbusError::ModbusException { code }),
            InjectedFault::AddressRange { .. } => unreachable!("handled above"),
        }
    }
}

impl Default for MockSlave {
    fn default() -> Self {
        Self::new(1)
    }
}

impl ModbusTransport for MockSlave {
    fn connect(&mut self, _host: &str, _port: u16) -> ModbusResult<()> {
        // Host is ignored — mock never dials the plant IP.
        match &self.fault {
            InjectedFault::Timeout => {
                let _ = self.take_fault();
                return Err(ModbusError::Timeout);
            }
            InjectedFault::Exception(_) => {
                let fault = self.take_fault();
                if let InjectedFault::Exception(code) = fault {
                    return Err(ModbusError::ModbusException { code });
                }
            }
            InjectedFault::ConnectionClosed => {
                let _ = self.take_fault();
                self.connected = false;
                return Err(ModbusError::ConnectionClosed);
            }
            InjectedFault::None | InjectedFault::AddressRange { .. } => {}
        }
        self.connected = true;
        Ok(())
    }

    fn disconnect(&mut self) {
        self.connected = false;
    }

    fn is_connected(&self) -> bool {
        self.connected
    }

    fn read_holding(&mut self, unit: u8, addr: u16, count: u16) -> ModbusResult<Vec<u16>> {
        validate_read_count(count)?;
        self.check_connected_and_unit(unit)?;
        self.apply_delay();
        self.eval_fault(addr, count)?;
        let mut out = Vec::with_capacity(count as usize);
        for offset in 0..count {
            let word = self.registers.get(&(addr + offset)).copied().unwrap_or(0);
            out.push(word);
        }
        self.read_count += 1;
        Ok(out)
    }

    fn write_multiple(&mut self, unit: u8, addr: u16, words: &[u16]) -> ModbusResult<()> {
        validate_write_count(words.len())?;
        self.check_connected_and_unit(unit)?;
        self.apply_delay();
        self.eval_fault(addr, words.len() as u16)?;
        for (i, word) in words.iter().enumerate() {
            self.registers.insert(addr + i as u16, *word);
        }
        self.write_count += 1;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lambda_catalog::{decode_raw, encode_raw, DataType};
    use lambda_modbus::{
        ClientConfig, ConcurrencyProbe, QueuedTransport, TcpModbusClient, MAX_READ_QUANTITY,
    };
    use std::sync::atomic::Ordering;
    use std::sync::Arc;
    use std::thread;
    use std::time::Duration;

    #[test]
    fn read_write_int16_roundtrip() {
        let mut mock = MockSlave::new(1);
        mock.connect("ignored", 502).unwrap();
        mock.write_multiple(1, 5005, &[210]).unwrap();
        assert_eq!(mock.read_holding(1, 5005, 1).unwrap(), vec![210]);
        assert_eq!(mock.write_count, 1);
        assert_eq!(mock.read_count, 1);
    }

    #[test]
    fn read_write_int32_high_word_first() {
        let mut mock = MockSlave::from_official_catalog();
        mock.connect("127.0.0.1", 502).unwrap();
        let value = 1_234_567_890_i32;
        let words = encode_raw(DataType::Int32, i64::from(value)).unwrap();
        // Heat-pump energy registers are INT32 in the official catalog (e.g. 1020).
        mock.write_multiple(1, 1020, &words).unwrap();
        let got = mock.read_holding(1, 1020, 2).unwrap();
        assert_eq!(decode_raw(DataType::Int32, &got).unwrap(), i64::from(value));
        assert_eq!(mock.get_i32(1020), Some(value));
    }

    #[test]
    fn exception_injection() {
        let mut mock = MockSlave::new(1);
        mock.connect("x", 502).unwrap();
        mock.inject_fault_once(InjectedFault::Exception(0x02));
        assert!(matches!(
            mock.read_holding(1, 0, 1),
            Err(ModbusError::ModbusException { code: 0x02 })
        ));
        // Cleared after one use.
        assert_eq!(mock.read_holding(1, 0, 1).unwrap(), vec![0]);
    }

    #[test]
    fn timeout_simulation() {
        let mut mock = MockSlave::new(1);
        mock.connect("x", 502).unwrap();
        mock.inject_fault(InjectedFault::Timeout);
        assert_eq!(mock.read_holding(1, 0, 1), Err(ModbusError::Timeout));
    }

    #[test]
    fn connection_closed_fault() {
        let mut mock = MockSlave::new(1);
        mock.connect("x", 502).unwrap();
        mock.inject_fault_once(InjectedFault::ConnectionClosed);
        assert_eq!(
            mock.write_multiple(1, 1, &[1]),
            Err(ModbusError::ConnectionClosed)
        );
        assert!(!mock.is_connected());
    }

    #[test]
    fn partial_block_address_range_failure() {
        let mut mock = MockSlave::new(1);
        mock.connect("x", 502).unwrap();
        mock.inject_fault(InjectedFault::AddressRange {
            start: 1000,
            count: 10,
            code: 0x02,
        });
        assert!(mock.read_holding(1, 0, 5).is_ok());
        assert!(matches!(
            mock.read_holding(1, 1005, 2),
            Err(ModbusError::ModbusException { code: 0x02 })
        ));
    }

    #[test]
    fn wrong_unit_rejected() {
        let mut mock = MockSlave::new(1);
        mock.connect("127.0.0.1", 502).unwrap();
        assert!(matches!(
            mock.read_holding(2, 0, 1),
            Err(ModbusError::ModbusException { code: 0x0B })
        ));
    }

    #[test]
    fn catalog_seed_covers_official_addresses() {
        let mock = MockSlave::from_official_catalog();
        let catalog = Catalog::official();
        assert!(mock.register_count_seeded() >= catalog.parameters.len());
        // HK1 / HK2 distinct setpoints present as seeded keys.
        assert!(mock.get(5005).is_some());
        assert!(mock.get(5105).is_some());
    }

    #[test]
    fn rejects_oversized_read() {
        let mut mock = MockSlave::new(1);
        mock.connect("x", 502).unwrap();
        assert!(mock.read_holding(1, 0, MAX_READ_QUANTITY + 1).is_err());
    }

    #[test]
    fn queued_concurrent_read_write_serialized() {
        let mut slave = MockSlave::new(1);
        slave.connect("x", 502).unwrap();
        let probe = ConcurrencyProbe::new(slave, Duration::from_millis(30));
        let max = Arc::clone(&probe.max_in_flight);
        let mut queued = QueuedTransport::new(probe);

        let q1 = queued.clone();
        let q2 = queued.clone();
        let h1 = thread::spawn(move || {
            let mut q = q1;
            q.read_holding(1, 0, 1).unwrap();
        });
        let h2 = thread::spawn(move || {
            let mut q = q2;
            q.write_multiple(1, 10, &[99]).unwrap();
        });
        h1.join().unwrap();
        h2.join().unwrap();
        assert_eq!(max.load(Ordering::SeqCst), 1);
        assert_eq!(queued.read_holding(1, 10, 1).unwrap(), vec![99]);
    }

    #[test]
    fn tcp_client_against_loopback_mock() {
        let mut slave = MockSlave::from_official_catalog();
        slave.seed_i16(5005, 425);
        slave.seed_i32(1020, 42_000);
        let server = spawn_tcp_mock_server(slave).unwrap();
        let port = server.port;

        let mut client = TcpModbusClient::new(ClientConfig::with_timeouts(
            Duration::from_millis(1000),
            Duration::from_millis(1000),
        ));
        client.connect("127.0.0.1", port).unwrap();
        assert_eq!(client.read_holding(1, 5005, 1).unwrap(), vec![425]);
        let words = client.read_holding(1, 1020, 2).unwrap();
        assert_eq!(decode_raw(DataType::Int32, &words).unwrap(), 42_000);

        client.write_multiple(1, 5005, &[350]).unwrap();
        assert_eq!(client.read_holding(1, 5005, 1).unwrap(), vec![350]);

        // Exception path via injected fault on shared slave.
        {
            let arc = server.slave();
            let mut g = arc.lock().unwrap();
            g.inject_fault_once(InjectedFault::Exception(0x03));
        }
        assert!(matches!(
            client.read_holding(1, 0, 1),
            Err(ModbusError::ModbusException { code: 0x03 })
        ));

        client.disconnect();
        server.shutdown();
    }
}
