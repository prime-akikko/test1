#!/usr/bin/env bash
# Minimal workspace check for agents/CI once crates exist.
# Safe no-op messaging if the Cargo workspace is not scaffolded yet (pre-M1).

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
  echo "==> cargo test --workspace"
  cargo test --workspace
fi

echo "scripts/check.sh: OK"
