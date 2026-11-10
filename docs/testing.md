# Test strategy (M10) — Mock-first, hardware local

## Why mock is mandatory

The Cloud Agent / typical CI runner **cannot** reach the plant IP `10.0.0.6`
(NF-06). Automated tests must never dial the live heat pump.

| Environment | Backend | Purpose |
| --- | --- | --- |
| Unit / CI / Cloud Agent | `lambda_mock` | Regression (catalog, queue, poll 5 s, cyclic 60 s, persist) |
| Local dev without WP | Mock (default feature) | UI smoke |
| Local LAN with EU13L | Real TCP (`--no-default-features --features gui`) | Hardware AC |

## Pyramid

| Layer | What | Where |
| --- | --- | --- |
| Unit | Addresses, scale, INT32, validation, JSON serde | `lambda_catalog`, `lambda_core` |
| Service | Queue, poll, cyclic, connect/disconnect | `lambda_core` + `lambda_mock` (+ FakeClock) |
| Persist | Temp-dir roundtrips, corrupt JSON fallback | `lambda_core` (`LAMBDA_MODBUS_GUI_DATA`) |
| UI state | Commands/Events → view-model | `lambda_app` (no GPU required) |
| Manual GUI | Shell, confirm dialog, Active panel | Windows local |
| Hardware | AC-01…AC-12 below | LAN on-site |

Fake clocks: production intervals are **5 s** / **60 s**; tests inject short
intervals and call `tick` — do not sleep wall-clock durations in CI.

## CI / Cloud commands

```bash
./scripts/check.sh          # fmt + cargo check --workspace
RUN_TESTS=1 ./scripts/check.sh
cargo test --workspace      # mock suite
# Do NOT:
# cargo run … against 10.0.0.6 from CI
```

GitHub Actions workflow (`.github/workflows/ci.yml`) runs fmt, check, and
`cargo test --workspace` on Linux. GUI feature is typechecked when feasible;
full window smoke remains a Windows acceptance path.

## Hardware checklist (local only)

Mirror of requirements §7. Run on a PC in the same LAN as the EU13L.

- [ ] **AC-01** App starts as a desktop window under **Windows** (GPUI).
- [ ] **AC-02** Defaults IP `10.0.0.6`, Port `502`, Unit ID `1`; all three editable and persisted under `%APPDATA%\LambdaModbusGui\`.
- [ ] **AC-03** Reachable WP → status connected; bad address → clear offline/error, no crash.
- [ ] **AC-04** Official map parameters visible (incl. HK1 + HK2); grouping OK.
- [ ] **AC-05** Display shows scaled value + unit + R/W; enums readable (label + raw).
- [ ] **AC-06** Auto refresh ~**5 s**; manual **Jetzt aktualisieren** works.
- [ ] **AC-07** Read-only fields not editable in UI.
- [ ] **AC-08** Confirmed write of a documented RW register changes the device; re-read shows new value (or documented device behaviour).
- [ ] **AC-09** Failed reads/writes show clear German errors.
- [ ] **AC-10** No cloud dependency for normal operation; LAN target only.
- [ ] **AC-11** Hardware acceptance is local (Cloud VM is not a hardware-AC environment).
- [ ] **AC-12** After confirmed cyclic setpoint write, value is rewritten every **~60 s** until cleared from Active; UI lists active rewrites.

### Live run (Windows CMD)

```bat
cd path\to\test1
cargo run -p lambda_app --no-default-features --features gui
```

Windows MSVC / D3D11 packaging notes are covered separately (M9 / `docs/windows-build.md`).
