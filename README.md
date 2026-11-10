# test1 — Lambda Modbus GPUI

Desktop GUI (Rust + GPUI) for a Lambda **EU13L** heat pump over Modbus TCP.

## Workspace (M1)

Cargo workspace at the repo root (**LambdaModbusGui**):

| Crate | Role |
| --- | --- |
| `crates/lambda_catalog` | Official register catalog + decode/encode helpers |
| `crates/lambda_modbus` | TCP client FC 0x03 / 0x10, queue, transport trait |
| `crates/lambda_core` | Commands/events, schedulers, config/active stores |
| `crates/lambda_mock` | In-memory Modbus slave for mock-first tests |
| `crates/lambda_app` | GPUI binary (Windows-primary); see crate README for GPUI pin |

```bash
cargo check --workspace
cargo test --workspace
./scripts/check.sh
# optional GUI link (needs native GPUI deps / Windows for real UI):
cargo check -p lambda_app --features gui
```

Locked defaults: IP `10.0.0.6`, Port `502`, Unit ID `1`, poll **5s**, cyclic writes **60s**, official register map only, German UI, Windows primary, mock-first tests.

## Agent setup

| Path | Purpose |
| --- | --- |
| [`AGENTS.md`](AGENTS.md) | Durable project instructions |
| [`.cursor/rules/`](.cursor/rules/) | Always-on / scoped Cursor rules |
| [`.cursor/skills/`](.cursor/skills/) | Reusable skills (catalog, GPUI UI, mock tests) |
| [`.cursor/hooks.json`](.cursor/hooks.json) + [`.cursor/hooks/`](.cursor/hooks/) | `rustfmt` after Rust edits |
| [`scripts/check.sh`](scripts/check.sh) | `cargo fmt --check` + `cargo check` |
| [`crates/lambda_app/README.md`](crates/lambda_app/README.md) | GPUI pin + Windows build notes |

Product specs live in the Project Context (`docs/requirements.md`, `docs/architecture.md`, `docs/implementation-plan.md`).
