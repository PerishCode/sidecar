# Phase: 1.2 - Align Operator and Release Surface

## Objective
Migrate `sidecar` onto the same repository and release operation shape as recent `flavor`: `flavor.toml` self-check baseline, `runseal` repo-local support entrypoints, and root `manage.sh|ps1` public install/update/uninstall managers. TCP broker implementation remains deferred until this surface is aligned.

## Exit Criteria
- [x] Add a TOML-first `flavor.toml` baseline and wire it into local/CI checks.
- [x] Add `runseal.toml` and repo-local wrappers/placeholders for support tasks, including Cloudflare mapping placeholders only.
- [x] Replace `scripts/manage/sidecar.{sh,ps1}` as the canonical public entrypoints with root `manage.sh|ps1`.
- [x] Update release publish/verify/smoke metadata and CLI `sidecar update` to use `manage.sh|ps1`.
- [x] Update README, AGENTS, init hooks, and pre-PR command docs.
- [x] Run focused verification and record any remaining external-token release/domain work.

## Work Log
- [2026-06-04T14:03:34Z] STARTED: Phase opened after confirming TCP broker should wait until operator/release migration completes.
- [2026-06-04T14:50:39Z] Migrated public manager entrypoints to root `manage.sh|ps1`; R2 publish metadata now emits `manager.unix/windows`; CLI update downloads `manage.*`.
- [2026-06-04T14:50:39Z] Added `flavor.toml`, `runseal.toml`, `.runseal/wrappers/cloudflare` placeholder, and wired flavor self-check into init/guard/release verify paths.
- [2026-06-04T14:50:39Z] Per aggressive alignment request, moved Rust inline tests into integration tests and split CLI command/render/runtime modules until flavor reports no issues.
- [2026-06-04T14:50:39Z] Verification passed: `cargo fmt --all --check`, `cargo clippy --locked --workspace --all-targets -- -D warnings`, `cargo test --locked --workspace`, `flavor check --root . --config flavor.toml`, `python3 scripts/init.py`, CLI doctor/plan smoke, shell syntax checks, and `runseal :cloudflare --help`.

## Technical Notes
- **Files Touched:** `.task/MAIN.md`, `.task/archive/PHASE_1.1.r1.md`, `.task/PHASE_1.2.md`, `flavor.toml`, `runseal.toml`, `.runseal/wrappers/cloudflare`, `manage.sh`, `manage.ps1`, `scripts/init.py`, README/AGENTS, release workflows/scripts, CLI update/output/commands modules, Rust integration tests.
- **New Dependencies:** `runseal`, `uv`, and `flavor v0.3.3+` become local development prerequisites.
- **Blockers:** None for repository migration. `sidecar.perish.uk` Cloudflare token/mapping remains a user-provided external step; current wrapper is placeholder-only.

---
*This phase will be popped/archived upon meeting exit criteria.*
