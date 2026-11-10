//! Official Lambda MODBUS-PROTOKOLL 1.0 register catalog (EU13L).
//!
//! Materialization: embedded JSON [`OFFICIAL_CATALOG_JSON`] parsed by
//! [`Catalog::official`]. See crate `README.md` for the pipeline and PDF source.
//!
//! Addresses use the manufacturer PDU formula:
//! `index * 1000 + subindex * 100 + datapoint` (0-based).

#![forbid(unsafe_code)]

mod blocks;
mod codec;

pub use blocks::{plan_all_read_blocks, plan_read_blocks, ReadBlock};
pub use codec::{
    assemble_int32, decode_display, decode_raw, display_to_raw, encode_display, encode_raw,
    enum_label, raw_to_display, split_int32,
};

use std::sync::OnceLock;

use serde::{Deserialize, Serialize};

/// Embedded official catalog JSON (MODBUS-PROTOKOLL 1.0, EU13L v1 instances).
pub const OFFICIAL_CATALOG_JSON: &str = include_str!("../data/official_catalog.json");

/// Catalog schema version for embedded metadata (not product release).
pub const SCHEMA_VERSION: u32 = 1;

/// Source identifier for the official protocol document.
pub const SOURCE: &str = "lambda-modbus-protokoll-1.0";

/// Target heat-pump model hint for v1.
pub const MODEL_HINT: &str = "EU13L";

/// Practical Modbus max quantity for a single FC 0x03 request.
pub const DEFAULT_MAX_READ_QUANTITY: u16 = 125;

/// Holding-register value encoding.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DataType {
    Int16,
    Uint16,
    /// Two consecutive registers, big-endian high-word-first.
    Int32,
}

/// Byte / word order for multi-register values (fixed by manufacturer).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ByteOrder {
    BigEndianHighWordFirst,
}

/// Read/write access from the official map.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Access {
    #[serde(rename = "r")]
    Read,
    #[serde(rename = "rw")]
    ReadWrite,
}

/// Controls Active-Setpoint / 60s rewrite after a confirmed first write.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PersistenceClass {
    /// Datapoint &lt; 50 — enter Active Setpoints after confirmed write.
    Cyclic,
    /// Datapoint ≥ 50 — no periodic rewrite unless explicitly activated.
    Retained,
}

/// Enum / status label pair.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EnumValue {
    pub raw: i64,
    pub label_de: String,
}

/// Module definition.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Module {
    pub module_id: String,
    pub index: u8,
    pub label_de: String,
    pub instances: Vec<Instance>,
}

/// Instance / subindex under a module.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Instance {
    pub subindex: u8,
    pub label_de: String,
    pub heating_circuit_index: Option<u8>,
}

/// Single parameter metadata row.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Parameter {
    pub param_id: String,
    pub module_id: String,
    pub group: String,
    pub heating_circuit_index: Option<u8>,
    /// Manufacturer module index (for address formula checks).
    pub index: u8,
    /// Manufacturer subindex.
    pub subindex: u8,
    /// Manufacturer datapoint number (00–99).
    pub datapoint: u8,
    pub address: u16,
    pub register_count: u8,
    pub data_type: DataType,
    pub byte_order: ByteOrder,
    pub scale: f64,
    pub unit: String,
    pub access: Access,
    pub label_de: String,
    pub description_de: Option<String>,
    pub range_min: Option<f64>,
    pub range_max: Option<f64>,
    pub enum_values: Option<Vec<EnumValue>>,
    pub write_group_id: Option<String>,
    pub persistence_class: PersistenceClass,
    /// Optional PDF page / section reference for review.
    pub source_ref: Option<String>,
}

/// Catalog root matching architecture §3.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Catalog {
    pub schema_version: u32,
    pub source: String,
    pub model_hint: String,
    pub modules: Vec<Module>,
    pub parameters: Vec<Parameter>,
}

/// Errors from catalog loading or encode/decode.
#[derive(Debug, Clone, PartialEq)]
pub enum CatalogError {
    Parse(String),
    WrongRegisterCount { expected: usize, got: usize },
    RawOutOfRange { raw: i64 },
    DisplayOutOfRange { display: f64, min: f64, max: f64 },
    InvalidScale { scale: f64 },
    NotFound { param_id: String },
}

impl std::fmt::Display for CatalogError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Parse(msg) => write!(f, "Katalog-Parsefehler: {msg}"),
            Self::WrongRegisterCount { expected, got } => {
                write!(
                    f,
                    "Falsche Registeranzahl: erwartet {expected}, erhalten {got}"
                )
            }
            Self::RawOutOfRange { raw } => write!(f, "Rohwert außerhalb des Typs: {raw}"),
            Self::DisplayOutOfRange { display, min, max } => {
                write!(f, "Anzeigewert {display} außerhalb [{min}, {max}]")
            }
            Self::InvalidScale { scale } => write!(f, "Ungültige Skala: {scale}"),
            Self::NotFound { param_id } => write!(f, "Parameter nicht gefunden: {param_id}"),
        }
    }
}

impl std::error::Error for CatalogError {}

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

impl Catalog {
    /// Parse catalog JSON (tests / alternate fixtures).
    pub fn from_json(json: &str) -> Result<Self, CatalogError> {
        serde_json::from_str(json).map_err(|e| CatalogError::Parse(e.to_string()))
    }

    /// Official EU13L catalog (lazy, process-wide).
    pub fn official() -> &'static Catalog {
        static CATALOG: OnceLock<Catalog> = OnceLock::new();
        CATALOG.get_or_init(|| {
            Catalog::from_json(OFFICIAL_CATALOG_JSON)
                .expect("embedded official_catalog.json must parse")
        })
    }

    pub fn empty() -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            source: SOURCE.into(),
            model_hint: MODEL_HINT.into(),
            modules: Vec::new(),
            parameters: Vec::new(),
        }
    }

    pub fn len(&self) -> usize {
        self.parameters.len()
    }

    pub fn is_empty(&self) -> bool {
        self.parameters.is_empty()
    }

    pub fn get(&self, param_id: &str) -> Option<&Parameter> {
        self.parameters.iter().find(|p| p.param_id == param_id)
    }

    pub fn by_address(&self, address: u16) -> Option<&Parameter> {
        self.parameters.iter().find(|p| p.address == address)
    }

    pub fn by_module(&self, module_id: &str) -> Vec<&Parameter> {
        self.parameters
            .iter()
            .filter(|p| p.module_id == module_id)
            .collect()
    }

    pub fn by_group(&self, group: &str) -> Vec<&Parameter> {
        self.parameters
            .iter()
            .filter(|p| p.group == group)
            .collect()
    }

    pub fn by_write_group(&self, write_group_id: &str) -> Vec<&Parameter> {
        self.parameters
            .iter()
            .filter(|p| p.write_group_id.as_deref() == Some(write_group_id))
            .collect()
    }

    /// Contiguous read blocks for all parameters (gap-aware).
    pub fn read_blocks(&self) -> Vec<ReadBlock> {
        plan_all_read_blocks(&self.parameters, DEFAULT_MAX_READ_QUANTITY)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::{HashMap, HashSet};

    /// Minimum official datapoint counts per module (PDF §3, excluding undefined 14–18).
    const MIN_COUNTS: &[(&str, usize)] = &[
        ("general", 5),
        ("emanager", 5),
        ("heatpump", 17),
        ("dhw", 7),
        ("buffer", 11),
        ("solar", 7),
        ("heating_circuit", 22), // HK1 + HK2
    ];

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
    fn official_catalog_loads() {
        let catalog = Catalog::official();
        assert_eq!(catalog.schema_version, SCHEMA_VERSION);
        assert_eq!(catalog.source, SOURCE);
        assert_eq!(catalog.model_hint, MODEL_HINT);
        assert!(!catalog.is_empty());
    }

    #[test]
    fn completeness_minimum_counts_per_module() {
        let catalog = Catalog::official();
        let mut counts: HashMap<&str, usize> = HashMap::new();
        for p in &catalog.parameters {
            *counts.entry(p.module_id.as_str()).or_default() += 1;
        }
        for &(module_id, min) in MIN_COUNTS {
            let got = counts.get(module_id).copied().unwrap_or(0);
            assert!(
                got >= min,
                "module {module_id}: expected >= {min} params, got {got}"
            );
            assert_eq!(
                got, min,
                "module {module_id}: exact official count expected {min}, got {got}"
            );
        }
        assert_eq!(catalog.len(), 74);
    }

    #[test]
    fn address_formula_and_unique_ids() {
        let catalog = Catalog::official();
        let mut ids = HashSet::new();
        let mut addrs = HashSet::new();
        for p in &catalog.parameters {
            let expected = pdu_address(p.index, p.subindex, p.datapoint);
            assert_eq!(p.address, expected, "{} address mismatch", p.param_id);
            assert!(
                ids.insert(p.param_id.clone()),
                "duplicate param_id {}",
                p.param_id
            );
            // INT32 shares the datapoint address only; low word is not a separate param.
            assert!(
                addrs.insert(p.address),
                "duplicate address {} for {}",
                p.address,
                p.param_id
            );
            assert_eq!(
                p.persistence_class,
                persistence_class_for_datapoint(p.datapoint)
            );
            let expected_count = match p.data_type {
                DataType::Int32 => 2,
                _ => 1,
            };
            assert_eq!(p.register_count, expected_count);
            assert_eq!(p.byte_order, ByteOrder::BigEndianHighWordFirst);
        }
    }

    #[test]
    fn hk1_and_hk2_addresses_distinct() {
        let catalog = Catalog::official();
        let hk1 = catalog.get("hk.1.flow_setpoint").expect("hk1 setpoint");
        let hk2 = catalog.get("hk.2.flow_setpoint").expect("hk2 setpoint");
        assert_eq!(hk1.address, 5005);
        assert_eq!(hk2.address, 5105);
        assert_eq!(hk1.heating_circuit_index, Some(1));
        assert_eq!(hk2.heating_circuit_index, Some(2));
        assert_eq!(hk1.group, "Heizkreis 1");
        assert_eq!(hk2.group, "Heizkreis 2");

        let hk1_addrs: HashSet<_> = catalog
            .by_group("Heizkreis 1")
            .iter()
            .map(|p| p.address)
            .collect();
        let hk2_addrs: HashSet<_> = catalog
            .by_group("Heizkreis 2")
            .iter()
            .map(|p| p.address)
            .collect();
        assert!(hk1_addrs.iter().all(|a| (5000..5100).contains(a)));
        assert!(hk2_addrs.iter().all(|a| (5100..5200).contains(a)));
        assert!(hk1_addrs.is_disjoint(&hk2_addrs));
        assert_eq!(hk1_addrs.len(), 11);
        assert_eq!(hk2_addrs.len(), 11);
    }

    #[test]
    fn v1_module_instances() {
        let catalog = Catalog::official();
        let expected = [
            ("general", 0u8, &[0u8][..]),
            ("emanager", 0, &[1]),
            ("heatpump", 1, &[0]),
            ("dhw", 2, &[0]),
            ("buffer", 3, &[0]),
            ("solar", 4, &[0]),
            ("heating_circuit", 5, &[0, 1]),
        ];
        assert_eq!(catalog.modules.len(), expected.len());
        for (module_id, index, subs) in expected {
            let m = catalog
                .modules
                .iter()
                .find(|m| m.module_id == module_id)
                .unwrap_or_else(|| panic!("missing module {module_id}"));
            assert_eq!(m.index, index);
            let got: Vec<u8> = m.instances.iter().map(|i| i.subindex).collect();
            assert_eq!(got, subs);
        }
        let hk = catalog
            .modules
            .iter()
            .find(|m| m.module_id == "heating_circuit")
            .unwrap();
        assert_eq!(hk.instances[0].heating_circuit_index, Some(1));
        assert_eq!(hk.instances[1].heating_circuit_index, Some(2));
    }

    #[test]
    fn buffer_write_group_06_to_08() {
        let catalog = Catalog::official();
        let group = catalog.by_write_group("buffer.request_temps");
        let dps: Vec<u8> = group.iter().map(|p| p.datapoint).collect();
        assert_eq!(dps, vec![6, 7, 8]);
        assert!(group.iter().all(|p| p.access == Access::ReadWrite));
        // Optional 09 is writable but not in the mandatory write group.
        let p09 = catalog.get("buffer.0.request_power").unwrap();
        assert!(p09.write_group_id.is_none());
        assert_eq!(p09.access, Access::ReadWrite);
    }

    #[test]
    fn no_community_only_undefined_wp_registers() {
        let catalog = Catalog::official();
        // PDF marks WP 14–18 as „Nicht definiert“; community maps invent request regs here.
        for addr in 1014u16..=1018 {
            assert!(
                catalog.by_address(addr).is_none(),
                "community-only address {addr} must not appear"
            );
        }
        // INT32 energy counters at 1020 / 1022 only.
        assert_eq!(
            catalog.by_address(1020).map(|p| p.data_type),
            Some(DataType::Int32)
        );
        assert_eq!(
            catalog.by_address(1022).map(|p| p.data_type),
            Some(DataType::Int32)
        );
        assert!(catalog.by_address(1021).is_none());
        assert!(catalog.by_address(1023).is_none());
    }

    #[test]
    fn enum_lookup_and_scale_helpers() {
        let catalog = Catalog::official();
        let mode = catalog.get("general.0.operating_mode").unwrap();
        let enums = mode.enum_values.as_ref().unwrap();
        assert_eq!(enum_label(enums, 1), Some("Automatik"));
        assert_eq!(enum_label(enums, 99), None);

        let outdoor = catalog.get("general.0.outdoor_temp").unwrap();
        let words = encode_display(outdoor, 12.3).unwrap();
        assert_eq!(words, vec![123]);
        assert!((decode_display(outdoor, &words).unwrap() - 12.3).abs() < 1e-9);
    }

    #[test]
    fn read_blocks_do_not_bridge_retained_gap() {
        let catalog = Catalog::official();
        let dhw = catalog.by_module("dhw");
        let blocks = plan_read_blocks(&dhw, DEFAULT_MAX_READ_QUANTITY);
        // 00–05 contiguous, then gap to retained 50.
        assert!(blocks.len() >= 2);
        assert_eq!(blocks[0].start, 2000);
        assert_eq!(blocks[0].count, 6);
        assert!(blocks.iter().any(|b| b.start == 2050 && b.count == 1));
    }

    #[test]
    fn official_read_blocks_cover_every_parameter() {
        let catalog = Catalog::official();
        let blocks = catalog.read_blocks();
        let mut covered = HashSet::new();
        for block in &blocks {
            for id in &block.param_ids {
                assert!(covered.insert(id.clone()), "param {id} in multiple blocks");
            }
            assert!(block.count <= DEFAULT_MAX_READ_QUANTITY);
            assert!(block.count >= 1);
        }
        assert_eq!(covered.len(), catalog.len());
    }
}
