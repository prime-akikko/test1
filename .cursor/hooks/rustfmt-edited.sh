#!/usr/bin/env bash
# afterFileEdit: format edited Rust files with rustfmt when available.
# Observational only — never block the agent. Always exit 0.

set -u

input="$(cat || true)"

file_path=""
if command -v jq >/dev/null 2>&1; then
  file_path="$(printf '%s' "$input" | jq -r '.file_path // empty' 2>/dev/null || true)"
else
  # Minimal fallback without jq
  file_path="$(printf '%s' "$input" | sed -n 's/.*"file_path"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p' | head -n1)"
fi

if [[ -z "${file_path}" || ! -f "${file_path}" ]]; then
  exit 0
fi

case "${file_path}" in
  *.rs) ;;
  *) exit 0 ;;
esac

if ! command -v rustfmt >/dev/null 2>&1; then
  exit 0
fi

# Prefer cargo rustfmt when a nearby Cargo.toml exists; else rustfmt directly.
dir="$(dirname "${file_path}")"
if command -v cargo >/dev/null 2>&1 && [[ -f "${dir}/Cargo.toml" || -f "Cargo.toml" ]]; then
  cargo fmt -- "${file_path}" >/dev/null 2>&1 || rustfmt --edition 2021 "${file_path}" >/dev/null 2>&1 || true
else
  rustfmt --edition 2021 "${file_path}" >/dev/null 2>&1 || true
fi

exit 0
