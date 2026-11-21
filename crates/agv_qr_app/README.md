# `agv_qr_app` — AGV QR localization test GUI

German GPUI **test / simulation** shell for the design in
[`docs/agv-qr-floor-localization.md`](../../docs/agv-qr-floor-localization.md).
Not a warehouse operator product UI — lab bench for pose + marker map + stub vision.

## GPUI pin

Same workspace pin as `lambda_app`: **`gpui = "=0.2.2"`** (see root `Cargo.toml`).
Feature `gui` is optional so Linux/Cloud `cargo check --workspace` stays green.

## Run

```bash
# Headless sim smoke (Cloud/CI — no camera, no native GUI libs)
cargo run -p agv_qr_app

# GPUI window (Windows primary; needs native GPUI deps)
cargo run -p agv_qr_app --features gui
```

Also:

```bash
cargo check -p agv_qr_app
cargo check -p agv_qr_app --features gui
cargo test -p agv_qr_core -p agv_qr_app
```

## UI (German)

1. **Simulierte Pose** — true \(x, y, \theta\), Zeit, Laufstatus  
2. **Geschätzte Pose** — stub vision output + Qualität  
3. **Letzte Erkennung** — RMSE, Ecken, Marker-ID, Alter  
4. **Markerkarte** — Lab 3×3 Grid (IDs + Positionen)  
5. **Steuerung** — Start / Stopp / Zurücksetzen  

Core crates: `agv_qr_core` (marker map, pose sink, sim engine, vision stub).
