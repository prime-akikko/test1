//! Modbus TCP MBAP + PDU framing for FC 0x03 and FC 0x10.

use crate::{ModbusError, ModbusResult, MAX_READ_QUANTITY, MAX_WRITE_QUANTITY};

pub const FC_READ_HOLDING: u8 = 0x03;
pub const FC_WRITE_MULTIPLE: u8 = 0x10;
pub const PROTOCOL_ID: u16 = 0;

/// Build a Modbus TCP ADU for Read Holding Registers (FC 0x03).
pub fn build_read_holding(
    transaction_id: u16,
    unit: u8,
    addr: u16,
    count: u16,
) -> ModbusResult<Vec<u8>> {
    validate_read_count(count)?;
    let mut buf = Vec::with_capacity(12);
    push_mbap(&mut buf, transaction_id, 6);
    buf.push(unit);
    buf.push(FC_READ_HOLDING);
    buf.extend_from_slice(&addr.to_be_bytes());
    buf.extend_from_slice(&count.to_be_bytes());
    Ok(buf)
}

/// Build a Modbus TCP ADU for Write Multiple Registers (FC 0x10).
pub fn build_write_multiple(
    transaction_id: u16,
    unit: u8,
    addr: u16,
    words: &[u16],
) -> ModbusResult<Vec<u8>> {
    validate_write_count(words.len())?;
    let quantity = words.len() as u16;
    let byte_count = (words.len() * 2) as u8;
    // Unit + FC + addr + qty + byte_count + data
    let pdu_len = 1 + 1 + 2 + 2 + 1 + words.len() * 2;
    let mut buf = Vec::with_capacity(6 + pdu_len);
    push_mbap(&mut buf, transaction_id, pdu_len as u16);
    buf.push(unit);
    buf.push(FC_WRITE_MULTIPLE);
    buf.extend_from_slice(&addr.to_be_bytes());
    buf.extend_from_slice(&quantity.to_be_bytes());
    buf.push(byte_count);
    for w in words {
        buf.extend_from_slice(&w.to_be_bytes());
    }
    Ok(buf)
}

fn push_mbap(buf: &mut Vec<u8>, transaction_id: u16, length: u16) {
    buf.extend_from_slice(&transaction_id.to_be_bytes());
    buf.extend_from_slice(&PROTOCOL_ID.to_be_bytes());
    buf.extend_from_slice(&length.to_be_bytes());
}

/// Parse a Modbus TCP response ADU for a read-holding request.
pub fn parse_read_holding_response(
    raw: &[u8],
    expect_transaction: u16,
    expect_unit: u8,
    expect_count: u16,
) -> ModbusResult<Vec<u16>> {
    let (unit, pdu) = split_adu(raw, expect_transaction)?;
    if unit != expect_unit {
        return Err(ModbusError::Other(format!(
            "Unit-ID-Mismatch: erwartet {expect_unit}, erhalten {unit}"
        )));
    }
    if pdu.is_empty() {
        return Err(ModbusError::Other("Leere PDU".into()));
    }
    if pdu[0] & 0x80 != 0 {
        return parse_exception(pdu);
    }
    if pdu[0] != FC_READ_HOLDING {
        return Err(ModbusError::Other(format!(
            "Unerwarteter Function Code 0x{:02X}",
            pdu[0]
        )));
    }
    if pdu.len() < 2 {
        return Err(ModbusError::Other("Read-Antwort zu kurz".into()));
    }
    let byte_count = pdu[1] as usize;
    if pdu.len() < 2 + byte_count {
        return Err(ModbusError::Other("Read-Antwort unvollständig".into()));
    }
    if byte_count != expect_count as usize * 2 {
        return Err(ModbusError::Other(format!(
            "Byte-Count {byte_count} passt nicht zu Quantity {expect_count}"
        )));
    }
    let mut words = Vec::with_capacity(expect_count as usize);
    for chunk in pdu[2..2 + byte_count].chunks_exact(2) {
        words.push(u16::from_be_bytes([chunk[0], chunk[1]]));
    }
    Ok(words)
}

/// Parse a Modbus TCP response ADU for a write-multiple request.
pub fn parse_write_multiple_response(
    raw: &[u8],
    expect_transaction: u16,
    expect_unit: u8,
    expect_addr: u16,
    expect_quantity: u16,
) -> ModbusResult<()> {
    let (unit, pdu) = split_adu(raw, expect_transaction)?;
    if unit != expect_unit {
        return Err(ModbusError::Other(format!(
            "Unit-ID-Mismatch: erwartet {expect_unit}, erhalten {unit}"
        )));
    }
    if pdu.is_empty() {
        return Err(ModbusError::Other("Leere PDU".into()));
    }
    if pdu[0] & 0x80 != 0 {
        return parse_exception(pdu);
    }
    if pdu[0] != FC_WRITE_MULTIPLE {
        return Err(ModbusError::Other(format!(
            "Unerwarteter Function Code 0x{:02X}",
            pdu[0]
        )));
    }
    if pdu.len() < 5 {
        return Err(ModbusError::Other("Write-Antwort zu kurz".into()));
    }
    let addr = u16::from_be_bytes([pdu[1], pdu[2]]);
    let qty = u16::from_be_bytes([pdu[3], pdu[4]]);
    if addr != expect_addr || qty != expect_quantity {
        return Err(ModbusError::Other(format!(
            "Write-ACK mismatch: addr {addr}/{expect_addr}, qty {qty}/{expect_quantity}"
        )));
    }
    Ok(())
}

fn split_adu(raw: &[u8], expect_transaction: u16) -> ModbusResult<(u8, &[u8])> {
    if raw.len() < 8 {
        return Err(ModbusError::Other("ADU zu kurz".into()));
    }
    let tid = u16::from_be_bytes([raw[0], raw[1]]);
    if tid != expect_transaction {
        return Err(ModbusError::Other(format!(
            "Transaction-ID-Mismatch: erwartet {expect_transaction}, erhalten {tid}"
        )));
    }
    let proto = u16::from_be_bytes([raw[2], raw[3]]);
    if proto != PROTOCOL_ID {
        return Err(ModbusError::Other(format!("Protocol-ID ungültig: {proto}")));
    }
    let length = u16::from_be_bytes([raw[4], raw[5]]) as usize;
    if raw.len() < 6 + length {
        return Err(ModbusError::Other("ADU Länge unvollständig".into()));
    }
    if length < 2 {
        return Err(ModbusError::Other("MBAP Length zu klein".into()));
    }
    let unit = raw[6];
    let pdu = &raw[7..6 + length];
    Ok((unit, pdu))
}

fn parse_exception<T>(pdu: &[u8]) -> ModbusResult<T> {
    let code = pdu.get(1).copied().unwrap_or(0);
    Err(ModbusError::ModbusException { code })
}

pub fn validate_read_count(count: u16) -> ModbusResult<()> {
    if count == 0 || count > MAX_READ_QUANTITY {
        Err(ModbusError::InvalidArgument(
            "Read-Quantity muss 1..=125 sein",
        ))
    } else {
        Ok(())
    }
}

pub fn validate_write_count(len: usize) -> ModbusResult<()> {
    if len == 0 || len > MAX_WRITE_QUANTITY as usize {
        Err(ModbusError::InvalidArgument(
            "Write-Quantity muss 1..=123 sein",
        ))
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn read_request_frame_layout() {
        let frame = build_read_holding(0x0007, 1, 5005, 2).unwrap();
        assert_eq!(
            frame,
            vec![
                0x00, 0x07, // tid
                0x00, 0x00, // proto
                0x00, 0x06, // length
                0x01, // unit
                0x03, // fc
                0x13, 0x8D, // addr 5005
                0x00, 0x02, // qty
            ]
        );
    }

    #[test]
    fn write_request_frame_layout() {
        let frame = build_write_multiple(1, 1, 5005, &[210]).unwrap();
        assert_eq!(
            &frame[7..],
            &[0x10, 0x13, 0x8D, 0x00, 0x01, 0x02, 0x00, 0xD2]
        );
    }

    #[test]
    fn parse_exception_code() {
        // tid=1, len=3, unit=1, fc=0x83, ex=0x02
        let raw = [0x00, 0x01, 0x00, 0x00, 0x00, 0x03, 0x01, 0x83, 0x02];
        let err = parse_read_holding_response(&raw, 1, 1, 1).unwrap_err();
        assert_eq!(err, ModbusError::ModbusException { code: 0x02 });
    }

    #[test]
    fn rejects_oversized_read() {
        assert!(build_read_holding(1, 1, 0, 126).is_err());
    }
}
