//! Serialized request queue over any [`ModbusTransport`].
//!
//! Poll and cyclic-write callers must share one queue so FC 0x03 and FC 0x10
//! never race on the same socket. Exclusive locking is the queue: every
//! connect/read/write/disconnect holds the mutex for the full request.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use crate::{ModbusError, ModbusResult, ModbusTransport};

/// Thread-safe wrapper that serializes all transport operations.
#[derive(Debug)]
pub struct QueuedTransport<T> {
    inner: Arc<Mutex<T>>,
}

impl<T> Clone for QueuedTransport<T> {
    fn clone(&self) -> Self {
        Self {
            inner: Arc::clone(&self.inner),
        }
    }
}

impl<T> QueuedTransport<T> {
    pub fn new(inner: T) -> Self {
        Self {
            inner: Arc::new(Mutex::new(inner)),
        }
    }

    /// Run a closure with exclusive access (for tests / diagnostics).
    pub fn with_inner<R>(&self, f: impl FnOnce(&mut T) -> R) -> ModbusResult<R> {
        let mut guard = self
            .inner
            .lock()
            .map_err(|_| ModbusError::Other("Request-Queue Mutex vergiftet".into()))?;
        Ok(f(&mut guard))
    }
}

impl<T: ModbusTransport> ModbusTransport for QueuedTransport<T> {
    fn connect(&mut self, host: &str, port: u16) -> ModbusResult<()> {
        self.with_inner(|t| t.connect(host, port))?
    }

    fn disconnect(&mut self) {
        let _ = self.with_inner(|t| t.disconnect());
    }

    fn is_connected(&self) -> bool {
        self.inner.lock().map(|g| g.is_connected()).unwrap_or(false)
    }

    fn read_holding(&mut self, unit: u8, addr: u16, count: u16) -> ModbusResult<Vec<u16>> {
        self.with_inner(|t| t.read_holding(unit, addr, count))?
    }

    fn write_multiple(&mut self, unit: u8, addr: u16, words: &[u16]) -> ModbusResult<()> {
        self.with_inner(|t| t.write_multiple(unit, addr, words))?
    }
}

/// Test helper: transport that tracks concurrent in-flight ops.
#[derive(Debug)]
pub struct ConcurrencyProbe<T> {
    pub inner: T,
    pub in_flight: Arc<std::sync::atomic::AtomicU32>,
    pub max_in_flight: Arc<std::sync::atomic::AtomicU32>,
    pub hold: Duration,
}

impl<T: ModbusTransport> ConcurrencyProbe<T> {
    pub fn new(inner: T, hold: Duration) -> Self {
        Self {
            inner,
            in_flight: Arc::new(std::sync::atomic::AtomicU32::new(0)),
            max_in_flight: Arc::new(std::sync::atomic::AtomicU32::new(0)),
            hold,
        }
    }

    fn enter(&self) {
        use std::sync::atomic::Ordering;
        let now = self.in_flight.fetch_add(1, Ordering::SeqCst) + 1;
        self.max_in_flight.fetch_max(now, Ordering::SeqCst);
        if !self.hold.is_zero() {
            std::thread::sleep(self.hold);
        }
    }

    fn leave(&self) {
        use std::sync::atomic::Ordering;
        self.in_flight.fetch_sub(1, Ordering::SeqCst);
    }
}

impl<T: ModbusTransport> ModbusTransport for ConcurrencyProbe<T> {
    fn connect(&mut self, host: &str, port: u16) -> ModbusResult<()> {
        self.enter();
        let r = self.inner.connect(host, port);
        self.leave();
        r
    }

    fn disconnect(&mut self) {
        self.enter();
        self.inner.disconnect();
        self.leave();
    }

    fn is_connected(&self) -> bool {
        self.inner.is_connected()
    }

    fn read_holding(&mut self, unit: u8, addr: u16, count: u16) -> ModbusResult<Vec<u16>> {
        self.enter();
        let r = self.inner.read_holding(unit, addr, count);
        self.leave();
        r
    }

    fn write_multiple(&mut self, unit: u8, addr: u16, words: &[u16]) -> ModbusResult<()> {
        self.enter();
        let r = self.inner.write_multiple(unit, addr, words);
        self.leave();
        r
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::Ordering;
    use std::thread;
    use std::time::Duration;

    /// Minimal in-crate stub so queue tests do not depend on `lambda_mock`.
    #[derive(Debug, Default)]
    struct StubSlave {
        connected: bool,
        regs: std::collections::BTreeMap<u16, u16>,
    }

    impl ModbusTransport for StubSlave {
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
        fn read_holding(&mut self, _unit: u8, addr: u16, count: u16) -> ModbusResult<Vec<u16>> {
            if !self.connected {
                return Err(ModbusError::NotConnected);
            }
            Ok((0..count)
                .map(|i| self.regs.get(&(addr + i)).copied().unwrap_or(0))
                .collect())
        }
        fn write_multiple(&mut self, _unit: u8, addr: u16, words: &[u16]) -> ModbusResult<()> {
            if !self.connected {
                return Err(ModbusError::NotConnected);
            }
            for (i, w) in words.iter().enumerate() {
                self.regs.insert(addr + i as u16, *w);
            }
            Ok(())
        }
    }

    #[test]
    fn concurrent_requests_are_serialized() {
        let probe = ConcurrencyProbe::new(StubSlave::default(), Duration::from_millis(30));
        let max = Arc::clone(&probe.max_in_flight);
        let mut queued = QueuedTransport::new(probe);
        queued.connect("127.0.0.1", 502).unwrap();

        let q1 = queued.clone();
        let q2 = queued.clone();
        let h1 = thread::spawn(move || {
            let mut q = q1;
            q.read_holding(1, 0, 1).unwrap();
        });
        let h2 = thread::spawn(move || {
            let mut q = q2;
            q.write_multiple(1, 1, &[42]).unwrap();
        });
        h1.join().unwrap();
        h2.join().unwrap();

        assert_eq!(
            max.load(Ordering::SeqCst),
            1,
            "queue must serialize concurrent read/write"
        );
        assert_eq!(queued.read_holding(1, 1, 1).unwrap(), vec![42]);
    }
}
