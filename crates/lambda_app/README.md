# `lambda_app` — Lambda Modbus GPUI

Windows-primary desktop binary for the Lambda **EU13L** Modbus TCP GUI.

## GPUI pin (M1 decision — 2026-11-10)

| Item | Value |
| --- | --- |
| Source | crates.io |
| Crate / version | `gpui = "=0.2.2"` (exact pin in workspace `Cargo.toml`) |
| Why not Git `main` | Pre-1.0 churn; lockfile must be reproducible |
| Rust | ≥ **1.85** (gpui 0.2.2 uses edition 2024); repo pins `rust-toolchain.toml` → **1.99.1** |
| Feature flag | `gui` — optional so Linux/Cloud `cargo check --workspace` stays green without native GUI libs |

Enable GPUI:

```bash
cargo check -p lambda_app --features gui
cargo run -p lambda_app --features gui
```

Default features enable `mock` only (in-memory backend for Cloud/CI).

## Windows build prerequisites (v1 target)

1. [rustup](https://rustup.rs/) target `x86_64-pc-windows-msvc`
2. Visual Studio Build Tools with **MSVC** C++ toolchain
3. Windows SDK
4. Direct3D 11 Feature Level **≥ 11.0** GPU/driver
5. From repo root:

```bash
cargo build -p lambda_app --release --features gui
```

Artefact: `target/release/lambda_app.exe` (plus assets once M6+ adds them).

Config directory (later M8): `%APPDATA%\LambdaModbusGui\`.

## Cloud / Linux note

This crate compiles without `gui` on developer Linux and Cloud Agents. Full windowing is a local Windows acceptance path (M6+). Do not point automated tests at live `10.0.0.6`.
