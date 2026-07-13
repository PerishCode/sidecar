# AGENTS

## Purpose

`sidecar` is the standalone home for an IPC-based sidecars project manager. It owns four product-neutral abstractions:

1. **Manifest-closed lifecycle** — `sidecar.toml` defines command/cwd/args/env/stamps/readiness/inspect/status/stop/reset for every target.
2. **Stamp args** — a packed `--sidecar-stamp=v=1;a=<app>;n=<namespace>;m=<mode>;s=<source>;e=<endpoint>` flag appended to every spawned target; it is the only sidecar launch metadata contract.
3. **Broker runtime** — one project/namespace-scoped loopback TCP broker discovered from `--sidecar-broker` argv identity plus live listener probing; targets receive the broker endpoint through the stamp `e` field.
4. **Inspect bridge** — a single-shot SidecarRuntime event frame over a Unix socket (TCP fallback) for talking to a running sidecar's inspect server.

This repository is not a `stim.io` module. `stim.io` and other consumers install `sidecar` as a published CLI through the R2-backed `manage.sh` / `manage.ps1` entrypoints.

## Product Boundary

`sidecar` is a local process control plane, not a cluster scheduler and not a local Kubernetes layer. It gives independent local workloads shared lifecycle, identity, discovery, inspect, and reset semantics while preserving their host filesystem, PATH, localhost, shell environment, credentials, and directly inspectable process shape.

Its core posture is:

- **Form isolation** — server, client, daemon, desktop, and dev-server targets are modeled as distinct workloads with their own wrapper, runtime config, status, readiness, logs, stop, and reset boundary.
- **No space isolation** — targets still run on the same host, user environment, filesystem, local tools, and loopback network. Do not introduce container image, pod, volume, network namespace, sandbox, or cluster assumptions into the product model.
- **Unified control plane** — sidecar owns namespace, identity stamps, dynamic endpoint injection, broker/runtime discovery, process lifecycle, inspect, reset, diagnostics, and data-home isolation.
- **Business unawareness** — product services should remain ordinary HTTP/Vite/Electron/Rust/CLI processes that consume argv, env, cwd, files, and endpoints. They should not need to understand the manifest, broker internals, stamp protocol, or sidecar runtime model.

The TCP broker is local service discovery and runtime registry for host processes. It is not a service mesh, cross-node scheduler, or container networking abstraction.

## Core Rules

- Keep `crates/core` product-neutral. No `stim`, `tauri`, chat, agent, or message-ledger semantics may leak in.
- Keep `crates/core` free of CLI output and process side effects. It exposes config (`Manifest`), diagnostics, plan, socket parser, stamp protocol, process discovery, and inspect client.
- Keep `crates/cli` as the installed binary boundary named `sidecar`.
- Manifest fields describe local process control-plane behavior only: start shape, cwd, args, env, readiness, identity, discovery, inspect, stop, reset, and data paths. Do not add product semantics or container/cluster scheduling semantics.
- `--config <path>` is the explicit manifest override. Without it, sidecar walks from cwd upward for the nearest `sidecar.toml`.
- Release assets are R2-backed. `SIDECAR_RELEASES_*` repo vars/secrets must be present before any release workflow can run.
- Consumer validation must use installed release assets, not `cargo install --path`, once a release exists.

## Update / Compatibility Policy

- The CLI never carries compatibility shims. Renaming or reshaping `Manifest`, CLI flags, the inspect protocol, the stamp protocol, or the installer surface is a hard cutover — no aliases, no deprecation warnings, no best-effort parsing of older shapes.
- No internal migrations: there is no `state v1 → v2` translator, no schema-version field, no auto-rewrite of user `sidecar.toml`. Older configs that no longer parse must hard-fail with an error pointing the user at the latest README.
- The escape hatch on any breakage is fixed and must always work: `sidecar reset` (kill stamped processes) → `manage.sh|ps1 uninstall` → reinstall the latest release → re-author `sidecar.toml` per the latest README. This single path replaces every other compatibility guarantee.
- Versioning is `0.Y.Z` indefinitely. A `Y` bump is breaking by default; pre-1.0 SemVer carries the unstable contract for us — we do not promote to `1.0.0`.
- The update mechanism itself follows the same rule: the startup check is best-effort and silently swallows every failure mode (network, parse, clock, missing curl); `sidecar update` is a thin wrapper around the manager (`manage.sh|ps1 update`) — it does not decompress, verify, or roll back.

## Build-time Stamps

`crates/cli` reads three optional build-time env vars via `option_env!` and bakes them into the binary; `.github/scripts/release/assets/package.{sh,ps1}` set all three from the release workflow:

- `SIDECAR_BUILD_VERSION` → `cli::version()` (defaults to `v<CARGO_PKG_VERSION>` for dev builds).
- `SIDECAR_BUILD_CHANNEL` → `cli::channel()` (`stable` / `beta` / `dev`; defaults to `dev`, which disables the startup check and `update` subcommand).
- `SIDECAR_BUILD_PUBLIC_URL` → fallback for the update check / subcommand when the runtime env var is absent.

The release workflows pass `RELEASE_CHANNEL` (`stable` for `release.yml`, `beta` for `release-beta.yml`) and the repo var `SIDECAR_RELEASES_PUBLIC_URL` into the build matrix steps so that every published binary is self-aware.

## Runtime Update Env Vars

- `SIDECAR_RELEASES_PUBLIC_URL` — overrides the build-time stamp for both check and update.
- `SIDECAR_CHANNEL` — overrides the build-time channel (e.g. flip a stable build to watch beta).
- `SIDECAR_NO_UPDATE_CHECK=1` — skip the startup check entirely.
- `SIDECAR_UPDATE_TTL=<n>[smhd]` — startup-check cache TTL; default `24h`, `0` = always fetch.

The update cache lives at `<data_home>/state/update-<channel>.json` (see Data Home below). It is single-key (`{checked_at, channel, latest_version}`) and may be deleted at any time.

## Data Home

Sidecar's persistent runtime state has a single canonical root, the data home:

- Default: `$XDG_DATA_HOME/sidecar` → `$HOME/.local/share/sidecar` on Unix, `%LOCALAPPDATA%\sidecar` on Windows.
- Layout:
  - `<data_home>/state/` — global, namespace-independent (currently: update cache).
  - `<data_home>/projects/<namespace>/` — per-project isolation (target pids, logs, runtime artifacts).

Override precedence (highest wins): `--data-home <path>` (CLI) > `SIDECAR_DATA_HOME` (env) > platform default. The manifest `[project].data_dir` field replaces the per-project subdir only (it does not move `state/`); `state/` always sits directly under `<data_home>`.

## Project Scoping (`-p` / `--project`)

The CLI accepts `-p <name>` / `--project <name>` (and `SIDECAR_PROJECT` env) as a Docker-Compose-style override of the manifest `[project].namespace`. It re-keys everything that's namespace-scoped in one shot:

- The stamp protocol's packed `n` namespace field on every spawned sidecar.
- The broker protocol's packed `n` namespace field on the project runtime broker.
- `discover_by_namespace` / `discover_by_app_namespace` lookups.
- The `<data_home>/projects/<namespace>/` subdir.

Precedence: CLI flag > env > manifest. The manifest value becomes a default; CLI always wins. This is what makes the same manifest run as multiple isolated projects on one machine.

## Reset Semantics (Escape Hatch)

`sidecar reset --config <path>` is the single escape hatch from any incompatible-change failure mode. It is signal-first by default: sidecar sends termination signals to sidecar-owned pids, observes whether they exit, and fails before deleting runtime data if they remain alive. `--force` is the explicit operator shortcut that escalates to force-kill after the graceful wait.

It:

1. Terminates every stamped process and every manifest-recorded target pid in the current namespace.
2. Terminates every broker process for the current project/namespace.
3. Removes `<data_home>/projects/<namespace>/` (manifest `data_dir` honored).
4. With `--all`: also removes `<data_home>/state/` (wipes update cache, etc.).

There is no `--keep-data` or confirm prompt by design — predictability and idempotency are reset's contract. Forceful cleanup is still opt-in through `--force`; it is an operator convenience, not the native lifecycle contract. The install root and bin link are out of scope for `reset` (they belong to `manage.sh|ps1 uninstall`). The fully-recovered state is: `sidecar reset --all --force` when graceful termination is insufficient → `manage.sh|ps1 uninstall` → reinstall latest → re-author `sidecar.toml` per the latest README.

## Installer Verbs

Root `manage.{sh,ps1}` accept exactly: `install`, `update`, `uninstall`. There is no `upgrade` alias. They default to `https://releases.sidecar.perish.uk` as the public release asset root, and `SIDECAR_RELEASES_PUBLIC_URL` / `--public-url` override it. The CLI's `sidecar update` subcommand downloads the canonical manager for the current channel and execs it with the `update` verb.

## Repo-local Support

`runseal.toml` and `.runseal/wrappers/*` are the repo-local operator entrypoints for support tasks that do not belong in the installable `sidecar` product binary. The wrappers are Deno TypeScript run through `runseal`; local development requires `runseal`, `deno`, and a `negentropy` binary matching the pin in `.runseal/negentropy.version`. Current support commands:

- `runseal :init` — idempotent post-clone validator. It quick-fails on missing required tools (git, deno, cargo, gh, runseal, the pinned negentropy) or repository entrypoints, and exits cleanly only when the checkout is ready for development.
- `runseal :guard` — the full local gate: fmt, clippy, tests, `deno fmt` / `deno check` over `.runseal`, and the pinned `negentropy --strict .`.
- `runseal :land` — lands the current clean topic branch: push, create or reuse the PR, await the checks on the exact pushed head SHA, squash-merge pinned to that SHA, sync `main`, delete the branch. `--dry-run` prints the plan without touching git or GitHub.
- `runseal :cloudflare` — repo-local Cloudflare support for checking credentials and ensuring exact-path `sidecar.perish.uk/manage.sh|ps1` redirects to the release bucket. Use `manage-ensure-redirect --dry-run` before applying changes.

## Constitution

`negentropy` is the structure checker for this repository; `negentropy --strict .` must print `clean` before anything lands. Its configuration is repo-owned:

- `negentropy.toml` — scan roots (`crates/**/*.rs`, `docs/**/*.md`), module roots, block/path depth limits, the comment ban, the single-word identifier rule, and the test-syntax grant for `crates/*/tests`.
- `vocabulary.toml` — registered compound atoms; while it is empty, every identifier must stay a single word.
- `.runseal/negentropy.version` — the pinned checker version (currently `v0.1.0-beta.9`). `runseal :init`, `runseal :guard`, and CI all verify the installed binary against this pin and refuse a mismatch.

## Common Commands

- Format: `cargo fmt --all --check`
- Test: `cargo test --locked --workspace`
- Clippy: `cargo clippy --locked --workspace --all-targets -- -D warnings`
- CLI smoke: `cargo run --locked -p cli -- doctor --config examples/minimal.toml`
- Plan: `cargo run --locked -p cli -- plan --config examples/minimal.toml --format json`
- Constitution check: `negentropy --strict .`
- Full gate: `runseal :guard`

## Repository Shape

- `crates/core/`: `Manifest` config, diagnostics, plan, socket parser, stamp protocol, process discovery, inspect client.
- `crates/cli/`: CLI parsing, lifecycle execution (`start`/`stop`/`restart`/`status`/`list`/`reset`), `inspect <sidecar> <event> [payload]`, output formatting, exit behavior.
- `manage.sh` and `manage.ps1`: public install/update/uninstall manager entrypoints uploaded as release assets.
- `docs/`: durable design notes for planned architecture changes, including the TCP broker runtime direction.
- `.runseal/`: runseal wrapper entrypoints (`guard.ts`, `init.ts`, `land.ts`, `cloudflare.seal`), the shared wrapper `lib/`, and the `negentropy.version` pin.
- `negentropy.toml` and `vocabulary.toml`: the constitution the `negentropy` checker enforces over `crates/` and `docs/`.
- `.github/scripts/`: workflow-only release helpers.

## Standard Workflow

### Initialize

After cloning or when the toolchain looks stale, run:

```bash
runseal :init
```

It validates the required tools (including a `negentropy` matching `.runseal/negentropy.version`) and the repository entrypoints, then exits. It installs nothing: there are no local git hooks. The gates are `runseal :guard` before landing and the `guard` workflow in CI.

### Branch Names

Use `<area>/<kebab-case-slug>`, where `<area>` matches the touched crate or concern. Examples:

- `cli/update-command`
- `core/process-discovery`
- `release/stable-dispatch`
- `docs/install-readme`

### Commit Messages

Subject: `<area>: <imperative summary>` on one line, ideally <= 72 characters. The body explains why the change is shaped this way first, then the concrete change list. End with any `Co-Authored-By:` trailers when pair-coded or agent-assisted.

### Pre-PR Checks

Every PR must pass the guard before review:

```bash
runseal :guard
```

It runs, in order:

```bash
cargo fmt --all --check
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo test --locked --workspace
deno fmt --check .runseal
deno check --config .runseal/deno.json --lock .runseal/deno.lock --frozen=true .runseal/wrappers/*.ts
negentropy --strict .
```

CI reruns the same wrapper: `.github/workflows/guard.yml` installs the pinned negentropy release, then executes `.runseal/wrappers/guard.ts` on every PR and every push to `main`.

### PR Descriptions

Use these top-level sections, in order:

```markdown
## Why
<what is broken or missing today>

## What
<concrete change list; reference filenames and modules>

## Tests
<commands run and results>
```

Add `## Compatibility` when a manifest field, CLI flag, protocol field, output shape, or exit-code behavior moves. Add `## Trade-off worth flagging` when the change has a downside that reviewers should hold in mind.

### Merging

`main` is PR-only and protected by the repository ruleset `main guard`. The required merge gate is the `guard` check from `.github/workflows/guard.yml` (currently `guard (ubuntu-latest)`). Required approvals are intentionally `0`.

From a clean topic branch, default to landing with:

```bash
runseal :land
```

It pushes the branch, creates or reuses the PR, records the exact pushed head SHA, polls the GitHub check-runs on that SHA until every one succeeds, squash-merges with `--match-head-commit <sha>` so only the audited commit can land, syncs `main`, and deletes the branch. If `:land` is unavailable, wait for green checks and fall back to `gh pr merge <num> --squash --delete-branch`.

## Stamp args protocol

Canonical flag name (consumers must accept and ignore it on their sidecar binaries):

```
--sidecar-stamp=v=1;a=<sidecar.name>;n=<project.namespace>;m=<sidecar.mode>;s=tool%3Asidecar;e=<runtime-endpoint>
```

The short keys are `v` (stamp protocol version), `a` (app/workload), `n` (namespace), `m` (mode), `s` (source), and `e` (sidecar runtime endpoint locator). Values are percent-encoded; for example `tool:sidecar` is encoded as `tool%3Asidecar`. Discovery uses only this flag via `ps -axo pid=,command=` on Unix and the Windows PowerShell `Win32_Process` query on Windows; the implementation is in `crates/core/src/runtime/process.rs`.

The stamp is the single source of truth for sidecar launch metadata. Do not add env fallbacks or sibling sidecar argv flags for control-plane metadata. Future sidecar launch fields must be encoded inside this stamp contract.

## Inspect bridge

Wire format (one line per direction):

```
request:  {"kind":"event","id":"...","verb":"...","payload":<json>}\n
response: {"kind":"event_response","id":"...","payload":<json>}\n
       or {"kind":"event_error","id":"...","error":{"code":"...","message":"..."}}\n
```

When CLI inspect is called without an explicit payload, the request payload is `{}` rather than `null`; typed project protocols should treat this as the unit/no-input event shape.

Default transport is Unix (`unix:///absolute/path.sock`). TCP is reserved for non-Unix fallback only.

The implementation is `crates/core/src/inspect.rs`. The CLI orchestration is `commands::inspect` in `crates/cli/src/commands.rs`.
