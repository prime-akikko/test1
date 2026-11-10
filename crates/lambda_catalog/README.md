# lambda_catalog

Official **Lambda MODBUS-PROTOKOLL 1.0** holding-register catalog for model **EU13L**.

## Materialization pipeline

1. Source of truth: manufacturer PDF
   [MODBUS-PROTOKOLL 1.0](https://www.lambda-wp.com/fileadmin/userdaten/docs/downloads/regler/1_0_0_Modbusprotokoll_2026.pdf).
2. Tabular extract reviewed against PDF §3.1–§3.7 (v1 instances only).
3. Embedded JSON: `data/official_catalog.json` (schema version 1).
4. Loaded at runtime by `Catalog::official()` via `include_str!` + `serde_json`.

Community maps (openHAB, ioBroker, …) are **cross-check only** and must not add
registers absent from the official table. Undefined WP datapoints 14–18 are omitted
on purpose (PDF: „Nicht definiert“).

## PDU addresses

`address = index * 1000 + subindex * 100 + datapoint` (0-based). Never store
`40001`-style Holding addresses as catalog truth.

## v1 instances

| Modul | Index | Subindex |
| --- | --- | --- |
| Allgemein | 0 | 0 |
| E-Manager | 0 | 1 |
| Wärmepumpe | 1 | 0 |
| Warmwasser | 2 | 0 |
| Puffer | 3 | 0 |
| Solar | 4 | 0 |
| Heizkreis 1 / 2 | 5 | 0 / 1 |

## Helpers

- Scale encode/decode (INT16 / UINT16 / INT32 high-word-first)
- Enum label lookup (German)
- Contiguous read-block planner (does not bridge address gaps)
- `write_group_id` for joint writes (`buffer.request_temps` = 06–08)
- `persistence_class`: datapoint &lt; 50 → `cyclic`, else `retained`
