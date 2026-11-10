---
name: modbus-register-catalog
description: Build and verify the official Lambda MODBUS-PROTOKOLL 1.0 register catalog (addresses, scale, enums, write groups, HK1/HK2). Use when editing lambda_catalog, catalog JSON/codegen, address formulas, or completeness vs the official PDF.
---

# Skill: Modbus register catalog

## When to use

- Materializing or changing `lambda_catalog`
- Address / scale / INT32 / enum decode-encode bugs
- Checking completeness against the official PDF (not community maps)

## Source of truth

- Official PDF: MODBUS-PROTOKOLL 1.0 (lambda-wp.com) — **acceptance only**
- Community maps (openHAB, ioBroker TSV, etc.) — cross-check only, never ship undocumented points
- Architecture catalog schema (modules, instances, `Parameter` fields)

## Address & type rules

1. PDU formula: `address = index * 1000 + subindex * 100 + datapoint` (0-based, 4-digit).
2. Examples: HK1 datapoint 05 → `5005`; HK2 → `5105`.
3. Never store Holding `40001`-style addresses as catalog truth.
4. Types: `int16` | `uint16` | `int32` (2 registers, high-word-first).
5. Scale from unit column (e.g. `[0.1 °C]` → scale `0.1`); Watt/Wh typically `1.0`.
6. `access`: `r` or `rw` only as documented.
7. `persistence_class`: datapoint &lt; 50 → `cyclic`; ≥ 50 → `retained`.
8. `write_group_id` for joint writes (at least buffer request temps 06–08).

## v1 instances (fixed)

| Module | Index | Subindexes |
| --- | --- | --- |
| Allgemein | 0 | 0 |
| E-Manager | 0 | 1 |
| Wärmepumpe | 1 | 0 |
| Warmwasser | 2 | 0 |
| Puffer | 3 | 0 |
| Solar | 4 | 0 |
| Heizkreis | 5 | 0 (HK1), 1 (HK2) |

Param IDs distinguish circuits: `hk.1.*` / `hk.2.*`. Labels German (`label_de`).

## Workflow

1. Extract/update tabular data from the official PDF (review the table excerpt before codegen).
2. Materialize via embedded JSON **or** const/codegen — pick one pipeline and document it in the crate.
3. Implement decode/encode helpers (raw ↔ display, enums with raw + DE label).
4. Build read-block planner: contiguous holdings per module/instance; do not bridge gaps.
5. Add unit tests: address formula, INT32 assemble, scale round-trip, enum lookup.
6. Completeness assert: minimum parameter counts per module (from PDF) via snapshot or count assert.
7. Reject any community-only register.

## Done checks

- [ ] All official datapoints for v1 instances present
- [ ] HK1 `50xx` and HK2 `51xx` distinct
- [ ] No community-only entries
- [ ] Unit tests green without network
