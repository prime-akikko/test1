//! Decode / encode helpers for official holding-register values.

use crate::{CatalogError, DataType, EnumValue, Parameter};

/// Assemble an INT32 from two Modbus words (high word first).
pub fn assemble_int32(high: u16, low: u16) -> i32 {
    ((u32::from(high) << 16) | u32::from(low)) as i32
}

/// Split an INT32 into high / low words (high word first).
pub fn split_int32(value: i32) -> (u16, u16) {
    let bits = value as u32;
    (((bits >> 16) & 0xFFFF) as u16, (bits & 0xFFFF) as u16)
}

/// Decode raw register words into a signed raw integer (pre-scale).
pub fn decode_raw(data_type: DataType, words: &[u16]) -> Result<i64, CatalogError> {
    match data_type {
        DataType::Int16 => {
            let w = *words.first().ok_or(CatalogError::WrongRegisterCount {
                expected: 1,
                got: words.len(),
            })?;
            Ok(i16::from_be_bytes(w.to_be_bytes()) as i64)
        }
        DataType::Uint16 => {
            let w = *words.first().ok_or(CatalogError::WrongRegisterCount {
                expected: 1,
                got: words.len(),
            })?;
            Ok(u64::from(w) as i64)
        }
        DataType::Int32 => {
            if words.len() < 2 {
                return Err(CatalogError::WrongRegisterCount {
                    expected: 2,
                    got: words.len(),
                });
            }
            Ok(i64::from(assemble_int32(words[0], words[1])))
        }
    }
}

/// Encode a signed raw integer into register words.
pub fn encode_raw(data_type: DataType, raw: i64) -> Result<Vec<u16>, CatalogError> {
    match data_type {
        DataType::Int16 => {
            if raw < i64::from(i16::MIN) || raw > i64::from(i16::MAX) {
                return Err(CatalogError::RawOutOfRange { raw });
            }
            Ok(vec![(raw as i16) as u16])
        }
        DataType::Uint16 => {
            if raw < 0 || raw > i64::from(u16::MAX) {
                return Err(CatalogError::RawOutOfRange { raw });
            }
            Ok(vec![raw as u16])
        }
        DataType::Int32 => {
            if raw < i64::from(i32::MIN) || raw > i64::from(i32::MAX) {
                return Err(CatalogError::RawOutOfRange { raw });
            }
            let (high, low) = split_int32(raw as i32);
            Ok(vec![high, low])
        }
    }
}

/// Apply scale: raw → display.
pub fn raw_to_display(raw: i64, scale: f64) -> f64 {
    (raw as f64) * scale
}

/// Inverse scale: display → raw (nearest integer).
pub fn display_to_raw(display: f64, scale: f64) -> Result<i64, CatalogError> {
    if scale == 0.0 || !scale.is_finite() {
        return Err(CatalogError::InvalidScale { scale });
    }
    Ok((display / scale).round() as i64)
}

/// Decode words for a parameter into a display value.
pub fn decode_display(param: &Parameter, words: &[u16]) -> Result<f64, CatalogError> {
    if words.len() < param.register_count as usize {
        return Err(CatalogError::WrongRegisterCount {
            expected: param.register_count as usize,
            got: words.len(),
        });
    }
    let raw = decode_raw(param.data_type, &words[..param.register_count as usize])?;
    Ok(raw_to_display(raw, param.scale))
}

/// Encode a display value into register words for a parameter.
pub fn encode_display(param: &Parameter, display: f64) -> Result<Vec<u16>, CatalogError> {
    if let (Some(min), Some(max)) = (param.range_min, param.range_max) {
        if display < min || display > max {
            return Err(CatalogError::DisplayOutOfRange { display, min, max });
        }
    }
    let raw = display_to_raw(display, param.scale)?;
    encode_raw(param.data_type, raw)
}

/// Look up a German enum label for a raw value.
pub fn enum_label<'a>(enums: &'a [EnumValue], raw: i64) -> Option<&'a str> {
    enums
        .iter()
        .find(|e| e.raw == raw)
        .map(|e| e.label_de.as_str())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Access, ByteOrder, PersistenceClass};

    fn sample_temp() -> Parameter {
        Parameter {
            param_id: "hk.1.flow_setpoint".into(),
            module_id: "heating_circuit".into(),
            group: "Heizkreis 1".into(),
            heating_circuit_index: Some(1),
            index: 5,
            subindex: 0,
            datapoint: 5,
            address: 5005,
            register_count: 1,
            data_type: DataType::Int16,
            byte_order: ByteOrder::BigEndianHighWordFirst,
            scale: 0.1,
            unit: "°C".into(),
            access: Access::ReadWrite,
            label_de: "Sollwert Vorlauftemperatur".into(),
            description_de: None,
            range_min: Some(7.0),
            range_max: Some(65.0),
            enum_values: None,
            write_group_id: None,
            persistence_class: PersistenceClass::Cyclic,
            source_ref: None,
        }
    }

    #[test]
    fn int32_high_word_first_roundtrip() {
        let value = 1_234_567_890_i32;
        let (high, low) = split_int32(value);
        assert_eq!(assemble_int32(high, low), value);
        let words = encode_raw(DataType::Int32, i64::from(value)).unwrap();
        assert_eq!(words, vec![high, low]);
        assert_eq!(
            decode_raw(DataType::Int32, &words).unwrap(),
            i64::from(value)
        );
    }

    #[test]
    fn scale_roundtrip_flow_setpoint() {
        let p = sample_temp();
        let words = encode_display(&p, 42.5).unwrap();
        assert_eq!(words, vec![425]);
        let display = decode_display(&p, &words).unwrap();
        assert!((display - 42.5).abs() < f64::EPSILON);
    }

    #[test]
    fn scale_rejects_out_of_range() {
        let p = sample_temp();
        assert!(matches!(
            encode_display(&p, 70.0),
            Err(CatalogError::DisplayOutOfRange { .. })
        ));
    }

    #[test]
    fn signed_int16_negative() {
        let words = encode_raw(DataType::Int16, -105).unwrap();
        assert_eq!(decode_raw(DataType::Int16, &words).unwrap(), -105);
        assert!((raw_to_display(-105, 0.1) + 10.5).abs() < f64::EPSILON);
    }
}
