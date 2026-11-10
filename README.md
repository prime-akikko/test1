# test1 — Lambda Modbus GPUI

Desktop GUI (Rust + GPUI) for a Lambda **EU13L** heat pump over Modbus TCP.

Application code lands in later milestones. This repo currently holds **Cursor agent setup** for the build process.

## Agent setup

| Path | Purpose |
| --- | --- |
| [`AGENTS.md`](AGENTS.md) | Durable project instructions |
| [`.cursor/rules/`](.cursor/rules/) | Always-on / scoped Cursor rules |
| [`.cursor/skills/`](.cursor/skills/) | Reusable skills (catalog, GPUI UI, mock tests) |
| [`.cursor/hooks.json`](.cursor/hooks.json) + [`.cursor/hooks/`](.cursor/hooks/) | `rustfmt` after Rust edits |
| [`scripts/check.sh`](scripts/check.sh) | `cargo fmt --check` + `cargo check` (no-op until workspace exists) |

Locked defaults: IP `10.0.0.6`, Port `502`, Unit ID `1`, poll **5s**, cyclic writes **60s**, official register map only, German UI, Windows primary, mock-first tests.

Product specs live in the Project Context (`docs/requirements.md`, `docs/architecture.md`, `docs/implementation-plan.md`).
