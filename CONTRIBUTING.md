# Contributing — Lambda Modbus GPUI

## Environments

| Path | Rule |
| --- | --- |
| **Cloud / CI** | **Mock only** (`lambda_mock`). Never target live `10.0.0.6`. |
| **Local without WP** | Default `cargo run -p lambda_app --features gui` (mock). |
| **Local with EU13L** | Same LAN; `--no-default-features --features gui`. |

Product locks (do not change casually): official register map only, Unit ID default 1,
poll 5 s, cyclic active rewrites 60 s, German UI, Windows-primary, write confirmation
on first write.

## Dev loop

```bash
./scripts/check.sh
RUN_TESTS=1 ./scripts/check.sh
cargo test --workspace
cargo check -p lambda_app --features gui   # optional native GUI link
```

## Docs

| Doc | Content |
| --- | --- |
| [`docs/testing.md`](docs/testing.md) | Test pyramid + hardware AC-01…AC-12 |
| [`docs/windows-build.md`](docs/windows-build.md) | Windows MSVC / D3D11 release notes |
| [`AGENTS.md`](AGENTS.md) | Agent / engineering rules |

Specs (requirements, architecture, implementation plan) live in the Project Context,
not necessarily in this git repo.
