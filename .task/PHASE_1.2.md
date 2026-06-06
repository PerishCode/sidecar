# Phase: 1.2 - Broker Lifecycle Integration

## Objective
Wire the broker substrate into lifecycle commands while keeping inspect
forwarding and target pid/log state unchanged.

## Exit Criteria
- [x] `start` ensures one broker for the resolved project/namespace.
- [x] Started targets receive `SIDECAR_RUNTIME_ENDPOINT`.
- [x] `status` and `list` show live broker pids/endpoint when discoverable.
- [x] full `stop` and `reset` terminate broker processes.
- [x] `stop <target>` terminates broker only when no other target remains running.
- [x] Add focused tests and local smoke coverage.

## Work Log
- [2026-06-06T11:58:43Z] STARTED: Phase initialized.
- [2026-06-06T12:08:53Z] Wired broker ensure/probe into `start`, runtime output into `status`/`list`, and broker termination into `stop`/`reset`.
- [2026-06-06T12:08:53Z] Fixed process-group signaling to use `kill -- -<pgid>` and added KILL fallback for stubborn process groups.
- [2026-06-06T12:08:53Z] Verification passed: `cargo fmt --all --check`, `cargo test --locked --workspace`, `cargo clippy --locked --workspace --all-targets -- -D warnings`, `flavor check --root . --config flavor.toml`, `cargo build --locked -p cli`, and temporary manifest lifecycle smoke.

## Technical Notes
- **Files Touched:** `.task/MAIN.md`, `.task/PHASE_1.2.md`, `README.md`, `AGENTS.md`, `docs/tcp-broker-runtime.md`, `crates/cli/src/commands.rs`, `crates/cli/src/commands/render.rs`, `crates/cli/src/commands/runtime.rs`, `crates/core/src/runtime/broker.rs`, `crates/core/src/runtime/process.rs`, package versions.
- **New Dependencies:** None expected beyond Phase 1.1.
- **Blockers:** None.

---
*This phase will be popped/archived upon meeting exit criteria.*
