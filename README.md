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

## Poll scheduler (M4)

`lambda_core::PollScheduler` reads all catalog parameters on a **5 s** interval (`defaults::POLL_INTERVAL_SECS` / `PollConfig::default`). Manual `read_all` / `read_group`, pause while disconnected, immediate poll on connect, and `last_success_at` are covered. Tests use `FakeClock` + `lambda_mock` (no wall-clock 5 s waits, no live WP).

## Cyclic write scheduler (M5)

`lambda_core::CyclicWriteScheduler` + `ActiveSetpointStore` rewrite confirmed **cyclic** setpoints every **60 s** (`defaults::CYCLIC_WRITE_INTERVAL_SECS` / `CyclicWriteConfig::default`). First write requires `confirmed: true`; cyclic repeats do not. Retained writes stay out of Active unless explicitly activated. `ClearActiveSetpoint` / `ClearAllActiveSetpoints` stop rewrites. Persistence path/model is ready; atomic file I/O is M8. Tests use `FakeClock` + `lambda_mock` (no wall-clock 60 s waits).

## GPUI shell (M6) + parameter R/W (M7)

`lambda_app` German desktop shell: connection bar, group navigation (incl. Heizkreis 1/2), parameter list with search, write flow with **„Wirklich schreiben?“**, Active-Setpoints panel (60 s cyclic via core). Default `mock` backend; enable windowing with `--features gui`. Live TCP: `--no-default-features --features gui`. See [`crates/lambda_app/README.md`](crates/lambda_app/README.md).

## Local persistence (M8)

Files under `%APPDATA%\LambdaModbusGui\` (Windows) or `~/.config/LambdaModbusGui` (Linux) / Application Support (macOS): `config.json`, `prefs.json`, `active_setpoints.json`. Atomic temp+rename writes. Override dir with env `LAMBDA_MODBUS_GUI_DATA` (Temp-Dir tests). Corrupt JSON → defaults + warning, no panic.

## Windows packaging (M9)

Release checklist for MSVC + D3D11 builds: [`docs/windows-build.md`](docs/windows-build.md).
Artefact: `target\release\lambda_app.exe` via
`cargo build -p lambda_app --release --features gui` (add `--no-default-features` for live TCP).

## Tests (mock-first) — M10


Cloud/CI uses `lambda_mock` (in-memory or loopback TCP). Do **not** point automated tests at live `10.0.0.6`. Hardware acceptance is local/self-hosted only.

- Strategy + AC checklist: [`docs/testing.md`](docs/testing.md)
- Contributor rules: [`CONTRIBUTING.md`](CONTRIBUTING.md)
- CI: `.github/workflows/ci.yml` (`cargo test --workspace`)

## Agent setup

| Path | Purpose |
| --- | --- |
| [`AGENTS.md`](AGENTS.md) | Durable project instructions |
| [`.cursor/rules/`](.cursor/rules/) | Always-on / scoped Cursor rules |
| [`.cursor/skills/`](.cursor/skills/) | Reusable skills (catalog, GPUI UI, mock tests) |
| [`.cursor/hooks.json`](.cursor/hooks.json) + [`.cursor/hooks/`](.cursor/hooks/) | `rustfmt` after Rust edits |
| [`scripts/check.sh`](scripts/check.sh) | `cargo fmt --check` + `cargo check` (`RUN_TESTS=1` adds tests) |
| [`crates/lambda_app/README.md`](crates/lambda_app/README.md) | GPUI pin + Windows build notes |
| [`docs/windows-build.md`](docs/windows-build.md) | M9 Windows release checklist |
| [`docs/testing.md`](docs/testing.md) | M10 mock-first strategy + hardware AC |
| [`CONTRIBUTING.md`](CONTRIBUTING.md) | Cloud=mock; hardware=self-hosted |

Product specs live in the Project Context (`docs/requirements.md`, `docs/architecture.md`, `docs/implementation-plan.md`).
