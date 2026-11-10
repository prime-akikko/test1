//! Contiguous holding-register read-block planner.

use crate::Parameter;

/// A contiguous FC 0x03 read request derived from the catalog.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReadBlock {
    /// First PDU address of the block.
    pub start: u16,
    /// Number of holding registers to read.
    pub count: u16,
    /// Parameter IDs covered by this block (in address order).
    pub param_ids: Vec<String>,
    /// Module / UI group label for diagnostics.
    pub group: String,
}

/// Plan read blocks for a sorted list of parameters.
///
/// Rules:
/// - Parameters must already share one module instance (caller filters).
/// - Contiguous addresses (including multi-register INT32 spans) merge.
/// - Gaps are **not** bridged — each gap starts a new block.
/// - Blocks never exceed `max_quantity` registers (Modbus practical max 125).
pub fn plan_read_blocks(params: &[&Parameter], max_quantity: u16) -> Vec<ReadBlock> {
    assert!(max_quantity >= 1, "max_quantity must be >= 1");

    let mut sorted: Vec<&Parameter> = params.to_vec();
    sorted.sort_by_key(|p| p.address);

    let mut blocks: Vec<ReadBlock> = Vec::new();
    for param in sorted {
        let start = param.address;
        let end = param
            .address
            .saturating_add(u16::from(param.register_count));

        if let Some(last) = blocks.last_mut() {
            let last_end = last.start.saturating_add(last.count);
            // Contiguous / overlapping: next param starts at or before end of current span.
            // A gap (start > last_end) must start a new block.
            if start <= last_end {
                let new_end = last_end.max(end);
                let new_count = new_end.saturating_sub(last.start);
                if new_count <= max_quantity {
                    last.count = new_count;
                    last.param_ids.push(param.param_id.clone());
                    continue;
                }
                // Would exceed max quantity — fall through to a new block.
            }
        }

        blocks.push(ReadBlock {
            start,
            count: u16::from(param.register_count),
            param_ids: vec![param.param_id.clone()],
            group: param.group.clone(),
        });
    }
    blocks
}

/// Plan read blocks for every module instance group in the catalog parameters.
pub fn plan_all_read_blocks(parameters: &[Parameter], max_quantity: u16) -> Vec<ReadBlock> {
    let mut by_group: Vec<(String, Vec<&Parameter>)> = Vec::new();
    for p in parameters {
        if let Some((_, list)) = by_group.iter_mut().find(|(g, _)| g == &p.group) {
            list.push(p);
        } else {
            by_group.push((p.group.clone(), vec![p]));
        }
    }
    let mut out = Vec::new();
    for (_, list) in by_group {
        out.extend(plan_read_blocks(&list, max_quantity));
    }
    out.sort_by_key(|b| b.start);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Access, ByteOrder, DataType, PersistenceClass};

    fn p(id: &str, addr: u16, count: u8, group: &str) -> Parameter {
        Parameter {
            param_id: id.into(),
            module_id: "test".into(),
            group: group.into(),
            heating_circuit_index: None,
            index: 0,
            subindex: 0,
            datapoint: (addr % 100) as u8,
            address: addr,
            register_count: count,
            data_type: if count == 2 {
                DataType::Int32
            } else {
                DataType::Int16
            },
            byte_order: ByteOrder::BigEndianHighWordFirst,
            scale: 1.0,
            unit: "—".into(),
            access: Access::Read,
            label_de: id.into(),
            description_de: None,
            range_min: None,
            range_max: None,
            enum_values: None,
            write_group_id: None,
            persistence_class: PersistenceClass::Cyclic,
            source_ref: None,
        }
    }

    #[test]
    fn merges_contiguous_registers() {
        let a = p("a", 1000, 1, "WP");
        let b = p("b", 1001, 1, "WP");
        let c = p("c", 1002, 1, "WP");
        let blocks = plan_read_blocks(&[&a, &b, &c], 125);
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].start, 1000);
        assert_eq!(blocks[0].count, 3);
    }

    #[test]
    fn does_not_bridge_gaps() {
        let a = p("a", 3000, 1, "Puffer");
        let b = p("b", 3001, 1, "Puffer");
        // gap at 3002..3049
        let c = p("c", 3050, 1, "Puffer");
        let blocks = plan_read_blocks(&[&a, &b, &c], 125);
        assert_eq!(blocks.len(), 2);
        assert_eq!(blocks[0].count, 2);
        assert_eq!(blocks[1].start, 3050);
    }

    #[test]
    fn int32_consumes_two_registers_then_continues() {
        let a = p("e", 1020, 2, "WP"); // covers 1020-1021
        let b = p("q", 1022, 2, "WP"); // covers 1022-1023
        let blocks = plan_read_blocks(&[&a, &b], 125);
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].start, 1020);
        assert_eq!(blocks[0].count, 4);
    }
}
