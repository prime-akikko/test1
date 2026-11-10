//! Tiny Modbus TCP loopback server backed by [`super::MockSlave`].
//! Used to exercise the real [`lambda_modbus::TcpModbusClient`] framing path.

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};

use lambda_modbus::{
    ModbusError, ModbusResult, ModbusTransport, FC_READ_HOLDING, FC_WRITE_MULTIPLE,
};

use crate::MockSlave;

/// Handle for a background TCP mock listener.
pub struct TcpMockServer {
    pub port: u16,
    slave: Arc<Mutex<MockSlave>>,
    join: Option<JoinHandle<()>>,
    shutdown: Arc<Mutex<bool>>,
}

impl TcpMockServer {
    pub fn slave(&self) -> Arc<Mutex<MockSlave>> {
        Arc::clone(&self.slave)
    }

    pub fn shutdown(mut self) {
        if let Ok(mut flag) = self.shutdown.lock() {
            *flag = true;
        }
        // Wake accept with a dummy connect.
        let _ = TcpStream::connect(("127.0.0.1", self.port));
        if let Some(h) = self.join.take() {
            let _ = h.join();
        }
    }
}

impl Drop for TcpMockServer {
    fn drop(&mut self) {
        if let Ok(mut flag) = self.shutdown.lock() {
            *flag = true;
        }
        let _ = TcpStream::connect(("127.0.0.1", self.port));
        if let Some(h) = self.join.take() {
            let _ = h.join();
        }
    }
}

/// Bind `127.0.0.1:0` and serve FC 0x03 / 0x10 against `slave`.
pub fn spawn_tcp_mock_server(slave: MockSlave) -> ModbusResult<TcpMockServer> {
    let listener = TcpListener::bind(("127.0.0.1", 0))
        .map_err(|e| ModbusError::Other(format!("TCP-Mock bind fehlgeschlagen: {e}")))?;
    let port = listener
        .local_addr()
        .map_err(|e| ModbusError::Other(e.to_string()))?
        .port();
    listener
        .set_nonblocking(false)
        .map_err(|e| ModbusError::Other(e.to_string()))?;

    let slave = Arc::new(Mutex::new(slave));
    {
        let mut g = slave
            .lock()
            .map_err(|_| ModbusError::Other("lock".into()))?;
        // Mark connected for in-process slave ops used by the handler.
        g.connect("127.0.0.1", port)?;
    }
    let slave_bg = Arc::clone(&slave);
    let shutdown = Arc::new(Mutex::new(false));
    let shutdown_bg = Arc::clone(&shutdown);

    let join = thread::spawn(move || {
        while !*shutdown_bg.lock().unwrap_or_else(|e| e.into_inner()) {
            match listener.accept() {
                Ok((mut stream, _)) => {
                    if *shutdown_bg.lock().unwrap_or_else(|e| e.into_inner()) {
                        break;
                    }
                    let _ = stream.set_read_timeout(Some(std::time::Duration::from_secs(2)));
                    let _ = stream.set_write_timeout(Some(std::time::Duration::from_secs(2)));
                    loop {
                        if let Err(_) = handle_one(&mut stream, &slave_bg) {
                            break;
                        }
                    }
                }
                Err(_) => break,
            }
        }
    });

    Ok(TcpMockServer {
        port,
        slave,
        join: Some(join),
        shutdown,
    })
}

fn handle_one(stream: &mut TcpStream, slave: &Arc<Mutex<MockSlave>>) -> Result<(), ()> {
    let mut header = [0u8; 7];
    stream.read_exact(&mut header).map_err(|_| ())?;
    let length = u16::from_be_bytes([header[4], header[5]]) as usize;
    if length < 2 {
        return Err(());
    }
    let mut rest = vec![0u8; length - 1];
    stream.read_exact(&mut rest).map_err(|_| ())?;
    let unit = header[6];
    let fc = rest[0];
    let tid = &header[0..2];

    let response_pdu = {
        let mut g = slave.lock().map_err(|_| ())?;
        match fc {
            FC_READ_HOLDING => {
                if rest.len() < 5 {
                    return Err(());
                }
                let addr = u16::from_be_bytes([rest[1], rest[2]]);
                let count = u16::from_be_bytes([rest[3], rest[4]]);
                match g.read_holding(unit, addr, count) {
                    Ok(words) => {
                        let mut pdu = Vec::with_capacity(2 + words.len() * 2);
                        pdu.push(FC_READ_HOLDING);
                        pdu.push((words.len() * 2) as u8);
                        for w in words {
                            pdu.extend_from_slice(&w.to_be_bytes());
                        }
                        pdu
                    }
                    Err(ModbusError::ModbusException { code }) => {
                        vec![FC_READ_HOLDING | 0x80, code]
                    }
                    Err(_) => vec![FC_READ_HOLDING | 0x80, 0x04],
                }
            }
            FC_WRITE_MULTIPLE => {
                if rest.len() < 6 {
                    return Err(());
                }
                let addr = u16::from_be_bytes([rest[1], rest[2]]);
                let qty = u16::from_be_bytes([rest[3], rest[4]]);
                let byte_count = rest[5] as usize;
                if rest.len() < 6 + byte_count || byte_count != qty as usize * 2 {
                    return Err(());
                }
                let mut words = Vec::with_capacity(qty as usize);
                for chunk in rest[6..6 + byte_count].chunks_exact(2) {
                    words.push(u16::from_be_bytes([chunk[0], chunk[1]]));
                }
                match g.write_multiple(unit, addr, &words) {
                    Ok(()) => {
                        let mut pdu = vec![FC_WRITE_MULTIPLE];
                        pdu.extend_from_slice(&addr.to_be_bytes());
                        pdu.extend_from_slice(&qty.to_be_bytes());
                        pdu
                    }
                    Err(ModbusError::ModbusException { code }) => {
                        vec![FC_WRITE_MULTIPLE | 0x80, code]
                    }
                    Err(_) => vec![FC_WRITE_MULTIPLE | 0x80, 0x04],
                }
            }
            other => vec![other | 0x80, 0x01],
        }
    };

    let mut adu = Vec::with_capacity(7 + response_pdu.len());
    adu.extend_from_slice(tid);
    adu.extend_from_slice(&0u16.to_be_bytes()); // protocol
    let len = (1 + response_pdu.len()) as u16;
    adu.extend_from_slice(&len.to_be_bytes());
    adu.push(unit);
    adu.extend_from_slice(&response_pdu);
    stream.write_all(&adu).map_err(|_| ())?;
    stream.flush().map_err(|_| ())?;
    Ok(())
}
