//! In-memory Modbus holding-register slave (skeleton — expanded in M3).
//!
//! Default automated-test backend; never targets live `10.0.0.6` from CI/Cloud.

#![forbid(unsafe_code)]

use std::collections::BTreeMap;

use lambda_modbus::{ModbusError, ModbusResult, ModbusTransport};

/// Simple register map keyed by PDU address.
#[derive(Debug, Default)]
pub struct MockSlave {
    connected: bool,
    unit_id: u8,
    registers: BTreeMap<u16, u16>,
    pub write_count: u64,
}

impl MockSlave {
    pub fn new(unit_id: u8) -> Self {
        Self {
            connected: false,
            unit_id,
            registers: BTreeMap::new(),
            write_count: 0,
        }
    }

    pub fn seed(&mut self, addr: u16, value: u16) {
        self.registers.insert(addr, value);
    }
}

impl ModbusTransport for MockSlave {
    fn connect(&mut self, _host: &str, _port: u16) -> ModbusResult<()> {
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
        if !self.connected {
            return Err(ModbusError::NotConnected);
        }
        if unit != self.unit_id {
            return Err(ModbusError::ModbusException { code: 0x0B });
        }
        let mut out = Vec::with_capacity(count as usize);
        for offset in 0..count {
            let word = self.registers.get(&(addr + offset)).copied().unwrap_or(0);
            out.push(word);
        }
        Ok(out)
    }

    fn write_multiple(&mut self, unit: u8, addr: u16, words: &[u16]) -> ModbusResult<()> {
        if !self.connected {
            return Err(ModbusError::NotConnected);
        }
        if unit != self.unit_id {
            return Err(ModbusError::ModbusException { code: 0x0B });
        }
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

    #[test]
    fn read_write_roundtrip() {
        let mut mock = MockSlave::new(1);
        mock.connect("127.0.0.1", 502).unwrap();
        mock.write_multiple(1, 5005, &[210]).unwrap();
        assert_eq!(mock.read_holding(1, 5005, 1).unwrap(), vec![210]);
        assert_eq!(mock.write_count, 1);
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
}
