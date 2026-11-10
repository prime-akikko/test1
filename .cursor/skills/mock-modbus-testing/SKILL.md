---
name: mock-modbus-testing
description: Mock-first Modbus testing for Lambda (in-memory slave, fake clock, queue/poll/cyclic coverage). Use when writing tests for lambda_mock, lambda_modbus, lambda_core schedulers, or CI without hardware.
---

# Skill: Mock Modbus testing

## When to use

- Implementing `lambda_mock` or transport trait tests
- Poll (5s) / cyclic write (60s) scheduler tests
- CI or Cloud Agent verification (hardware unreachable)

## Why mock is mandatory

Cloud VMs and CI cannot reach `10.0.0.6`. Hardware AC is local/self-hosted only. Default every automated test to the mock unless the user explicitly runs a LAN hardware suite.

## Transport trait

Test against the same trait as production, e.g.:

- `connect` / `disconnect`
- `read_holding(unit, addr, count)` → FC 0x03 semantics
- `write_multiple(unit, addr, words)` → FC 0x10 semantics

Production TCP impl and mock both implement it. Core depends on the trait.

## Mock minimum capabilities

- In-memory register map seeded from catalog defaults
- Unit ID check (wrong unit → exception/reject)
- Inject: timeout, connection closed, Modbus exception, partial block failure
- Count write calls (assert cyclic rewrite frequency under fake clock)
- Write groups as one FC 0x10 frame (optional mid-group fail only if testing framing)

## Fake clock

- Do **not** sleep real 5s / 60s in unit tests.
- Inject short intervals (e.g. 50ms) or manual ticks.
- Assert “N ticks → N polls / N cyclic rewrites”.

## Required scenarios

| Area | Assert |
| --- | --- |
| Catalog helpers | Address, INT32, scale round-trip (no network) |
| Client + mock | Read INT16/INT32, write + re-read, exception, timeout |
| Queue | Concurrent read/write requests serialize |
| Poll | ≥2 cycles update values; pause offline; resume on connect; partial failure marks errors |
| Cyclic | Cyclic write after user confirm repeats; retained does not; clear stops; rewrite error keeps Active entry |
| Persist | Temp-dir roundtrips for config/prefs/active; corrupt JSON safe fallback |

## Environment matrix

| Environment | Backend |
| --- | --- |
| Unit / CI / Cloud | `lambda_mock` |
| Local without WP | Mock or local TCP mock listener |
| LAN with EU13L | Real TCP client (manual AC) |

## Anti-patterns

- Pointing CI at live plant IP
- Waiting wall-clock 60s for scheduler proofs
- Bypassing the transport trait with ad-hoc sockets in core tests
- Silent drop of Active entries on cyclic write failure

## Done checks

- [ ] Suite passes without network / without WP
- [ ] Fake clock used for both schedulers
- [ ] README or test docs state: Cloud/CI = mock; hardware = self-hosted
