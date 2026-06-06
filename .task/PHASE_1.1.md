# Phase: 1.1 - Core Broker Runtime Substrate

## Objective
Implement the product-neutral broker runtime foundation: broker identity codec,
per-pid TCP listener discovery, hidden runtime serve command, and minimal
hello/hello_ok handshake.

## Exit Criteria
- [x] Add and test broker argv identity encoding/decoding in core.
- [x] Add and test TCP listener discovery by pid across supported platforms.
- [x] Add a hidden internal broker serve command.
- [x] Implement and test broker hello handshake.
- [x] Keep inspect forwarding, target pid/log state, and lifecycle integration deferred to later phases.

## Work Log
- [2026-06-06T11:51:58Z] STARTED: Phase initialized.
- [2026-06-06T11:58:09Z] Added `core::runtime::{broker,process,tcp}` and hidden `sidecar runtime serve <project> <namespace>`.
- [2026-06-06T11:58:09Z] Verification passed: `cargo test --locked --workspace`, `cargo clippy --locked --workspace --all-targets -- -D warnings`, `flavor check --root . --config flavor.toml`, and real broker hello smoke.

## Technical Notes
- **Files Touched:** `.task/MAIN.md`, `.task/PHASE_1.1.md`, `Cargo.toml`, `Cargo.lock`, `crates/core/src/runtime/*`, `crates/core/src/lib.rs`, `crates/core/src/stamp.rs`, `crates/cli/src/broker_runtime.rs`, `crates/cli/src/cli.rs`, `crates/cli/src/lib.rs`, integration tests.
- **New Dependencies:** `windows-sys` for Windows TCP owner-pid listener discovery.
- **Blockers:** None. macOS first slice may use `lsof` as documented.

---
*This phase will be popped/archived upon meeting exit criteria.*
