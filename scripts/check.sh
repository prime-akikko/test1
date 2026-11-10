#!/usr/bin/env bash
# Workspace check for agents/CI (mock-first — never dials 10.0.0.6).
# Set RUN_TESTS=1 to also run `cargo test --workspace` (recommended in CI).

set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${root}"

if [[ ! -f Cargo.toml ]]; then
  echo "scripts/check.sh: no Cargo.toml yet (workspace not scaffolded). Skipping."
  exit 0
fi

echo "==> cargo fmt --all -- --check"
cargo fmt --all -- --check

echo "==> cargo check --workspace"
cargo check --workspace

if [[ "${RUN_TESTS:-0}" == "1" ]]; then
  echo "==> cargo test --workspace (mock)"
  cargo test --workspace
fi

echo "scripts/check.sh: OK"
