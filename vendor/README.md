# Vendored patches

## `xattr` 0.2.3

Stock crates.io `xattr 0.2.3` references `libc::ENOATTR`, which recent `libc`
releases no longer export on `linux-gnu` (use `ENODATA`, the historical alias).

GPUI 0.2.2 pulls this crate on Unix via `zed-async-tar`. Without the patch,
`cargo check -p lambda_app --features gui` fails on modern Linux toolchains
even though Windows (primary target) never builds the unix `xattr` path.

Patch: on Linux/Android, define `ENOATTR` as `libc::ENODATA`.
