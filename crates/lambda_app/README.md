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

## M6 shell (German UI)

Layout:

1. **Verbindung** — Host / Port / Unit-ID (Defaults `10.0.0.6` / `502` / `1`), buttons **Verbinden** / **Trennen** / **Jetzt aktualisieren**
2. **Gruppen** — Allgemein, E-Manager, Wärmepumpe, Warmwasser, Puffer, Solar, **Heizkreis 1**, **Heizkreis 2**
3. **Parameter** — placeholder until M7 (R/W + „Wirklich schreiben?“)
4. **Aktive Sollwerte** — panel present (empty until confirmed writes in M7)
5. **Status** — Verbindungsstatus, letzter Poll, zyklischer Hinweis, Mock/TCP-Backend

Core wiring: `AppSession` drives `PollScheduler` over mock (default) or TCP; UI state maps `ConnectionChanged` / `Poll*` events. Cyclic Active-Setpoint **writes** remain M7; the panel/status slots are already in the shell.

## Windows build prerequisites (v1 target)

1. [rustup](https://rustup.rs/) target `x86_64-pc-windows-msvc`
2. Visual Studio Build Tools with **MSVC** C++ toolchain
3. Windows SDK
4. Direct3D 11 Feature Level **≥ 11.0** GPU/driver
5. From repo root:

```bash
cargo build -p lambda_app --release --features gui
```

Artefact: `target/release/lambda_app.exe`.

Config directory (later M8): `%APPDATA%\LambdaModbusGui\`.

## Cloud / Linux notes

| Path | Expectation |
| --- | --- |
| `cargo check --workspace` / `cargo test -p lambda_app` | Green without `gui` (headless smoke + state/session tests) |
| `cargo check -p lambda_app --features gui` | Compiles GPUI on Linux when system GUI deps + vendored `xattr` patch are present; **does not** require a display for typecheck |
| Full window smoke | Local **Windows** acceptance; Wayland/X11 + Vulkan may work on Linux but are not v1 AC |

Workspace `[patch.crates-io] xattr` (see `vendor/xattr`) fixes a `libc::ENOATTR` breakage that otherwise fails Unix `gui` builds with recent `libc`. Windows never compiles that unix dependency.

Do not point automated tests at live `10.0.0.6`.
