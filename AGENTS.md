# Lambda Modbus GPUI — Agent Instructions

Desktop GUI (Rust + GPUI) for a **Lambda EU13L** heat pump over **Modbus TCP**.
Product source of truth lives in the Project Context docs (requirements, architecture, implementation plan). Do not redesign product scope here.

## Locked product decisions (v1)

| Topic | Decision |
| --- | --- |
| Model | Lambda **EU13L** |
| Register map | **Official** MODBUS-PROTOKOLL 1.0 only (no community-only registers) |
| Defaults | IP `10.0.0.6`, Port `502`, Unit ID `1` |
| Poll / read | Every **5 seconds** (+ manual refresh) |
| Cyclic writes | Active user setpoints rewritten every **60 seconds** |
| First write | Explicit confirmation dialog (“Wirklich schreiben?”); cyclic repeats need no new confirm |
| Heating circuits | Exactly **HK1 + HK2** (index 5, subindex 0 and 1) |
| Platform | **Windows** primary (MSVC + Windows SDK, D3D11); Linux/macOS optional, not v1 AC |
| UI language | **German** strings |
| Tests | **Mock-first**; real WP is LAN-only and unreachable from Cloud VMs |

## Stack & crate layout

Target workspace (name **LambdaModbusGui**):

- `lambda_catalog` — official register catalog + decode/encode
- `lambda_modbus` — TCP client FC **0x03** / **0x10**, request queue, transport trait
- `lambda_core` — commands/events, poll + cyclic schedulers, config/active stores
- `lambda_mock` — in-memory Modbus slave for tests
- `lambda_app` — GPUI binary (Windows-primary)

Dependency direction: `lambda_app` → `lambda_core` → `lambda_modbus` (+ `lambda_catalog`); `lambda_mock` only for tests/features.

## Hard engineering rules

1. **Official map only** — PDU addresses (`index * 1000 + subindex * 100 + datapoint`). Never ship `40001`-style addresses as catalog truth.
2. **Holding registers only** — read FC 0x03, write FC 0x10 (even for a single register).
3. **INT32** = two registers, big-endian high-word-first.
4. **Serialized I/O** — one request queue on the Modbus client; poll and cyclic writes must not race the socket.
5. **Safety** — `WriteRegister` / `WriteRegisterGroup` require `confirmed: true` for first write; RO fields are never editable; no blind bulk rewrite of all RW registers.
6. **persistence_class** — datapoint &lt; 50 → `cyclic` (enter Active Setpoints after confirmed write); ≥ 50 → `retained` (no periodic rewrite unless explicitly activated).
7. **Write groups** — honor manufacturer joint-write groups (e.g. buffer request temps 06–08) via `write_group_id`.
8. **Persist locally** under `%APPDATA%\LambdaModbusGui\` (`config.json`, `prefs.json`, `active_setpoints.json`); atomic temp+rename. No cloud DB.
9. **GPUI pin** — pin crates.io or a known Git revision; commit lockfile; do not float on `main`.
10. **German UI** — user-visible labels, dialogs, errors, and status text in German.

## Testing expectations

- Default path: `lambda_mock` + fake clock (do not wait real 5s/60s in unit tests).
- CI / Cloud Agent: mock only; never target live `10.0.0.6` from CI.
- Hardware acceptance (AC checklist) is local/self-hosted only.

## Skills (use when relevant)

| Skill | When |
| --- | --- |
| `.cursor/skills/modbus-register-catalog` | Catalog schema, addresses, scaling, completeness vs official PDF |
| `.cursor/skills/gpui-modbus-ui` | GPUI shell, German UI, write-confirm, active-setpoints panel |
| `.cursor/skills/mock-modbus-testing` | Mock slave, fake clock, scheduler/queue tests |

## Hooks & scripts

- `.cursor/hooks.json` — after Rust edits, run `rustfmt` when available.
- `scripts/check.sh` — `cargo fmt --check` + `cargo check --workspace` (once the workspace exists).

## Out of scope (v1)

Cloud/VPN remote control, multi-device, mobile/web UI, Modbus RTU, autodiscovery, historical DB/charts, firmware update, undocumented community registers, Linux/macOS as acceptance blockers.

## Working style for agents

- Prefer small milestones (M1–M10 in the implementation plan); one independently testable step at a time.
- Do not invent registers, timers, or safety UX that contradict the locked decisions above.
- When product docs and code disagree, **docs win** until an explicit product change is approved.
