//! Official Lambda register catalog (skeleton — filled in M2).
//!
//! Addresses use the manufacturer PDU formula:
//! `index * 1000 + subindex * 100 + datapoint` (0-based).

#![forbid(unsafe_code)]

/// Catalog schema version for embedded metadata (not product release).
pub const SCHEMA_VERSION: u32 = 1;

/// Source identifier for the official protocol document.
pub const SOURCE: &str = "lambda-modbus-protokoll-1.0";

/// Target heat-pump model hint for v1.
pub const MODEL_HINT: &str = "EU13L";

/// Holding-register value encoding.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DataType {
    Int16,
    Uint16,
    /// Two consecutive registers, big-endian high-word-first.
    Int32,
}

/// Read/write access from the official map.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Access {
    Read,
    ReadWrite,
}

/// Controls Active-Setpoint / 60s rewrite after a confirmed first write.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PersistenceClass {
    /// Datapoint &lt; 50 — enter Active Setpoints after confirmed write.
    Cyclic,
    /// Datapoint ≥ 50 — no periodic rewrite unless explicitly activated.
    Retained,
}

/// Module definition (placeholder until M2 materializes the PDF table).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Module {
    pub module_id: &'static str,
    pub index: u8,
    pub label_de: &'static str,
}

/// Instance / subindex under a module.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Instance {
    pub subindex: u8,
    pub label_de: &'static str,
    pub heating_circuit_index: Option<u8>,
}

/// Single parameter metadata row (placeholder).
#[derive(Debug, Clone, PartialEq)]
pub struct Parameter {
    pub param_id: &'static str,
    pub module_id: &'static str,
    pub group: &'static str,
    pub heating_circuit_index: Option<u8>,
    pub address: u16,
    pub register_count: u8,
    pub data_type: DataType,
    pub scale: f64,
    pub unit: &'static str,
    pub access: Access,
    pub label_de: &'static str,
    pub write_group_id: Option<&'static str>,
    pub persistence_class: PersistenceClass,
}

/// PDU address: `index * 1000 + subindex * 100 + datapoint`.
pub fn pdu_address(index: u8, subindex: u8, datapoint: u8) -> u16 {
    u16::from(index) * 1000 + u16::from(subindex) * 100 + u16::from(datapoint)
}

/// Default persistence class from the datapoint number.
pub fn persistence_class_for_datapoint(datapoint: u8) -> PersistenceClass {
    if datapoint < 50 {
        PersistenceClass::Cyclic
    } else {
        PersistenceClass::Retained
    }
}

/// Empty catalog handle for later M2 population.
#[derive(Debug, Default, Clone)]
pub struct Catalog {
    pub parameters: Vec<Parameter>,
}

impl Catalog {
    pub fn empty() -> Self {
        Self::default()
    }

    pub fn len(&self) -> usize {
        self.parameters.len()
    }

    pub fn is_empty(&self) -> bool {
        self.parameters.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pdu_address_hk1_and_hk2() {
        assert_eq!(pdu_address(5, 0, 5), 5005);
        assert_eq!(pdu_address(5, 1, 5), 5105);
    }

    #[test]
    fn persistence_class_threshold() {
        assert_eq!(
            persistence_class_for_datapoint(49),
            PersistenceClass::Cyclic
        );
        assert_eq!(
            persistence_class_for_datapoint(50),
            PersistenceClass::Retained
        );
    }

    #[test]
    fn empty_catalog_compiles() {
        let catalog = Catalog::empty();
        assert!(catalog.is_empty());
        assert_eq!(SCHEMA_VERSION, 1);
        assert_eq!(MODEL_HINT, "EU13L");
    }
}
