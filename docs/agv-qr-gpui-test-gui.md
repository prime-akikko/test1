# AGV QR — GPUI Test-GUI (Simulation)

How to run the lab/test GUI for [agv-qr-floor-localization.md](./agv-qr-floor-localization.md).

This is a **simulation / bring-up** tool. It does **not** expand the Lambda Modbus product
scope and does **not** require a live camera in Cloud/CI.

## Crates

| Crate | Role |
| --- | --- |
| `crates/agv_qr_core` | Marker map, pose output interface, stub vision, sim engine |
| `crates/agv_qr_app` | German GPUI shell (`gui` feature) + headless smoke |

Architecture stubs match the design doc: pose sink, marker map store, vision pipeline trait
(real OpenCV/camera later).

## Commands

```bash
# Default: headless mock sim (CI / Linux without display)
cargo run -p agv_qr_app

# Windows-primary GUI (same GPUI 0.2.2 pin as lambda_app)
cargo run -p agv_qr_app --features gui

cargo test -p agv_qr_core -p agv_qr_app
cargo check --workspace
```

See also [`crates/agv_qr_app/README.md`](../crates/agv_qr_app/README.md).
