# sidecar

Lightweight, manifest-driven process instance manager for projects that need to run a small set of cooperating local processes.

`sidecar` turns an explicit config file into a validated process plan, lifecycle commands, process discovery, reset behavior, and an optional inspect IPC channel. It is intentionally product-agnostic: consumers own their manifests, product semantics, and inspect server implementations.

The core mechanisms are:

- **Manifest-closed lifecycle**: command, cwd, args, env, readiness, inspect socket, status identity, stop behavior, and reset boundary are declared up front.
- **Packed stamp identity**: spawned processes receive one compact `--sidecar-stamp=...` arg so they can be discovered and managed later.
- **Inspect bridge**: a single SidecarRuntime event frame over a Unix socket, with TCP reserved for fallback probes.

## Why

`sidecar` came out of a Tauri-adjacent development problem: one local app often needs a few cooperating processes nearby, such as a frontend shell, a backend service, a provider process, or an inspect server. Starting them is easy; keeping their identity, readiness, logs, namespace isolation, status, stop, and reset behavior predictable is the part that tends to spread into ad hoc scripts.

This repository keeps that machinery small and product-neutral. It is not trying to become a general supervisor, service mesh, or deployment system. It is a narrow local tool for projects that need repeatable multi-process instances without baking product-specific meaning into the process manager.

## Install

Release installation is R2-backed.

```sh
curl -fsSL "$SIDECAR_RELEASES_PUBLIC_URL/stable/latest/manage.sh" \
  | sh -s -- install --channel stable --public-url "$SIDECAR_RELEASES_PUBLIC_URL"
```

Beta releases use the same manager with `--channel beta`. The planned
`sidecar.perish.uk` release mapping is intentionally not automated in this
checkout yet; keep using `SIDECAR_RELEASES_PUBLIC_URL` until that mapping is
provisioned.

## Local Smoke

After cloning, initialize the local checkout:

```sh
python3 scripts/init.py
```

Local initialization expects `flavor v0.3.3+`, `runseal`, and `uv` to be
available.

Run the fast local smoke path:

```sh
cargo run --locked -p cli -- doctor --config examples/minimal.toml
cargo run --locked -p cli -- plan   --config examples/minimal.toml --format json
flavor check --root . --config flavor.toml
```

## Release

Stable releases are started from the `release-stable` workflow (`.github/workflows/release.yml`). The workflow resolves the Cargo version against R2 metadata, runs verification, publishes artifacts and managers to R2, then creates the git tag after publish succeeds.

Beta releases are started from `release-beta`. The workflow advances `vX.Y.Z-beta.N` from R2 beta metadata unless a version override is provided.

## Boundary

- `crates/core` owns `Manifest` config, diagnostics, plan generation, socket parsing, stamp args protocol, process discovery, and the inspect IPC client.
- `crates/cli` owns the installed `sidecar` command surface (lifecycle + inspect).
- Consumers own product-specific manifest files and the actual inspect server implementations on their sidecars.

## Manifest Model

`sidecar.toml` is the lifecycle contract, not a launch snippet. A target's command, cwd, args, static env, stamp delivery, readiness, inspect socket, status identity, stop behavior, and reset boundary must be derivable from the manifest plus product-neutral sidecar rules.

Top-level shape:

- `[project]`: `name`, `namespace`, optional `root`, optional `data_dir`
- `[[sidecars]]`: background service targets, launched in declaration order
- `[app]`: optional foreground app target, launched after sidecars
- per target: `name`, `command`, `args`, `cwd`, `mode`, `env`, `stamp_via_env`, `inspect_socket`, `endpoint_env`, `inherits_env`, `ready`

`inspect_socket` supports `{project}`, `{namespace}`, and `{name}` templates.

## Stamp Args Protocol

A consumer that uses `sidecar` to manage a process must accept (and ignore) the canonical packed stamp arg appended to its command line:

```
--sidecar-stamp=a=<sidecar.name>;n=<project.namespace>;m=<sidecar.mode>;s=tool%3Asidecar
```

The short keys are `a` (app), `n` (namespace), `m` (mode), and `s` (source); values are percent-encoded. This lets `sidecar` discover, status-check, and stop running sidecars cross-platform. Targets that cannot accept extra argv must set `stamp_via_env = true`; `sidecar` then records their pid in project state and injects stamp env for consumers that need it.

## Inspect Bridge

`sidecar inspect <sidecar> <event> [<json-payload>]` connects to the target's `inspect_socket` and sends one SidecarRuntime event frame. Longer-running provider checks can pass `--inspect-timeout <seconds>`:

- request:  `{"kind":"event","id":"...","verb":"...","payload":<json>}\n`
- response: `{"kind":"event_response","id":"...","payload":<json>}\n`
- error:    `{"kind":"event_error","id":"...","error":{"code":"...","message":"..."}}\n`

Unix sockets are the canonical transport (`unix:///absolute/path.sock`). TCP (`tcp://host:port`) is reserved for non-Unix fallback or explicit compatibility probes.

Report parser gaps, diagnostics noise, install issues, and missing capabilities at:

https://github.com/PerishCode/sidecar/issues
