# Windows Build & Packaging Notes (M9)

v1 target: **Windows 10/11 x64**. This checklist is enough to produce a runnable
`lambda_app.exe` locally. A full installer (WiX / MSIX) is **out of scope** for
v1 acceptance — only mentioned as a later option.

## 1. Toolchain

1. Install [rustup](https://rustup.rs/).
2. Ensure the MSVC toolchain and Windows target:

```bat
rustup default stable
rustup target add x86_64-pc-windows-msvc
```

Repo pins `rust-toolchain.toml` (currently **1.99.1**). rustup will auto-install it.

3. Install **Visual Studio Build Tools** (or full VS) with workload:
   - **Desktop development with C++**
   - Windows 10/11 SDK
4. Open a **Developer Command Prompt** / `x64 Native Tools` prompt (or ensure
   `link.exe` is on `PATH`).

## 2. GPUI / GPU requirements

| Item | Requirement |
| --- | --- |
| UI crate | `gpui = "=0.2.2"` (workspace pin; do not float on Zed `main`) |
| Feature | build with `--features gui` |
| Graphics | **Direct3D 11**, Feature Level **≥ 11.0** |
| Driver | Current GPU driver; remote desktop / VMs without D3D11 FL 11.0 may fail at runtime |

Known pitfalls:

- Building with GNU/`x86_64-pc-windows-gnu` is unsupported for GPUI on Windows — use **MSVC**.
- Missing Windows SDK → `windows` crate link errors.
- Cloud/Linux CI compiles without `gui` by default; Linux `gui` needs system GUI deps and the vendored `vendor/xattr` patch (ENOATTR).

## 3. Build commands

From the repo root:

```bat
:: Debug GUI (mock backend — safe without WP)
cargo run -p lambda_app --features gui

:: Release GUI against live Modbus TCP (same LAN as EU13L)
cargo build -p lambda_app --release --no-default-features --features gui
```

Artefact:

```text
target\release\lambda_app.exe
```

No extra asset files are required in v1 (no bundled icons/fonts yet).

Optional later (not v1 AC): `cargo-wix`, MSIX packaging, code signing.

## 4. First run

1. Start `lambda_app.exe`.
2. Config directory is created on first persist:

```text
%APPDATA%\LambdaModbusGui\
  config.json
  prefs.json
  active_setpoints.json
```

3. Windows Firewall may prompt for outbound LAN access — allow **TCP port 502**
   to the heat pump (`10.0.0.6` by default).
4. Defaults in the UI: host `10.0.0.6`, port `502`, unit ID `1`.

## 5. Hardware acceptance

Abnahme against the real EU13L is **LAN-only / self-hosted**. Cloud VMs cannot
reach `10.0.0.6`. Use the hardware checklist in [`testing.md`](testing.md).

## 6. Quick verify (developer)

```bat
cargo fmt --all -- --check
cargo check --workspace
cargo test --workspace
cargo check -p lambda_app --features gui
```

Or: `scripts\check.sh` from Git Bash / WSL (fmt + check).
