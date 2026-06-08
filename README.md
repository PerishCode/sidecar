# sidecar

Lightweight, manifest-driven process instance manager for projects that need to run a small set of cooperating local processes.

`sidecar` turns a manifest file into a validated process plan, lifecycle commands, process discovery, reset behavior, and an optional inspect IPC channel. It is intentionally product-agnostic: consumers own their manifests, product semantics, and inspect server implementations.

The core mechanisms are:

- **Manifest-closed lifecycle**: command, cwd, args, env, readiness, inspect socket, status identity, stop behavior, and reset boundary are declared up front.
- **Packed stamp contract**: spawned processes receive one compact `--sidecar-stamp=...` arg that carries sidecar-owned identity and runtime endpoint metadata.
- **Broker runtime endpoint**: each project/namespace gets one local TCP broker, discovered from process argv plus live listener probing, and encoded into spawned target stamps.
- **Inspect bridge**: a single SidecarRuntime event frame over a Unix socket, with TCP reserved for fallback probes.

## Why

Local native workloads share one host: PATH, shell environment, credentials,
filesystem, localhost, ports, IPC paths, logs, caches, and runtime data. Once a
project has more than one communicating process, that shared space needs a
machine-readable runtime identity. Without it, dev servers, daemons, UI shells,
workers, packaged apps, worktrees, release channels, and agent sessions can
cross wires: wrong endpoints, mixed state, stale pids, ambiguous logs, mistaken
tests, and unsafe cleanup.

`sidecar` gives those local processes shallow isolation without space
isolation. It names workloads, stamps process identity, injects runtime
endpoints, tracks readiness/status, offers inspect/reset hooks, and keeps
namespace-scoped runtime state. It deliberately does not care what the process
does: UI, daemon, provider, worker, dev server, agent, and packaged helper are
all just native host processes.

The goal is low-friction local communication and cleanup, not product semantics.
Consumers own the business topology and any business shutdown behavior; sidecar
owns the product-neutral process facts it can observe.

## When

Use `sidecar` when a local application or toolchain is really a named runtime
made of several native processes:

- Multiple local workloads need to discover each other through injected
  endpoints or IPC paths.
- The same repo may run across dev, packaged, stable, beta, nightly, PR, or
  worktree contexts on one machine.
- Human operators and agents may run or validate local runtimes concurrently.
- A process should remain business-unaware and only consume argv, env, cwd,
  files, and endpoints.
- Start, ready, status, inspect, stop, logs, runtime data, and reset need to be
  scoped by namespace instead of guessed from ports or process names.

Electron and Tauri make this topology obvious, but the need is not tied to
those stacks. A fully native app can hide the same layered runtime behind one
language or one launcher. If communicating host processes need identity,
discovery, lifecycle observation, and namespace-scoped cleanup, `sidecar` is a
fit.

## Not Fit

Do not use `sidecar` when the real requirement is:

- A truly single-process app with no runtime topology to name, discover,
  inspect, or reset.
- Container image builds, pod scheduling, volume/network namespace modeling, or
  cluster orchestration.
- Security isolation, permission sandboxing, tenant isolation, or host
  hardening. Sidecar provides identity and shallow runtime boundaries, not a
  security boundary.
- A production process supervisor, monitoring daemon, crash-loop manager, or
  alerting system.
- Business-level cleanup guarantees. Sidecar judges lifecycle outcomes by
  sidecar-owned process facts, especially whether the known pids for the target
  and namespace are gone.

Forceful cleanup is an opt-in operator shortcut. By default, lifecycle handling
is signal-first: sidecar signals or invokes the configured stop path, observes
the pids it created or can identify, and reports the result. If a caller's stop
entrypoint starts unrelated processes, leaks unstamped children, or performs
business-specific cleanup incorrectly, that remains the caller's responsibility
unless those processes are represented through sidecar identity.

## Install

Release installation is R2-backed.

```sh
curl -fsSL https://sidecar.perish.uk/manage.sh \
  | sh -s -- install --channel stable
```

Beta releases use the same manager with `--channel beta`. The manager defaults
to `https://releases.sidecar.perish.uk` as its release asset root. The
`sidecar.perish.uk` mapping is maintained through `runseal :cloudflare`.

## Local Smoke

After cloning, initialize the local checkout:

```sh
runseal :init
```

Local initialization expects `flavor v0.3.3+` and `runseal v0.6.0+` to be
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

When `--config <path>` is omitted, `sidecar` walks from the current directory
upward to find the nearest `sidecar.toml`. Explicit `--config` always wins.
Discovered configs are printed on stderr as `sidecar: using config <path>`.
If no manifest is found, sidecar fails and prints the searched paths.

Top-level shape:

- `[project]`: `name`, `namespace`, optional `root`, optional `data_dir`
- `[[sidecars]]`: background service targets, launched in declaration order
- `[app]`: optional foreground app target, launched after sidecars
- per target: `name`, `command`, `args`, `cwd`, `mode`, `env`, `inspect_socket`, `inherits_env`, `ready`

`inspect_socket` supports `{project}`, `{namespace}`, and `{name}` templates.

## Broker Runtime

`sidecar start` ensures one local broker for the resolved project and namespace
before launching targets. The broker is identified by argv:

```text
--sidecar-broker=p=<project.name>;n=<project.namespace>;s=tool%3Asidecar
```

The broker endpoint is not written to state files or injected into env.
`sidecar` discovers it by finding the broker process, inspecting TCP listeners
owned by that pid, and confirming the listener with a hello handshake. Started
targets receive the live endpoint as part of the packed stamp's `e` field:

```text
--sidecar-stamp=v=1;a=<name>;n=<namespace>;m=<mode>;s=tool%3Asidecar;e=tcp%3A%2F%2F127.0.0.1%3A<port>
```

`status` and `list` include a `runtime` object in JSON output and a `runtime:`
line in text output. Full `stop` and `reset` terminate broker processes; `stop
<target>` terminates the broker only when no other target in the namespace is
still running.

## Stamp Args Protocol

A consumer that uses `sidecar` to manage a process must accept (and ignore) the
canonical packed stamp arg appended to its command line:

```
--sidecar-stamp=v=1;a=<sidecar.name>;n=<project.namespace>;m=<sidecar.mode>;s=tool%3Asidecar;e=<runtime-endpoint>
```

The short keys are `v` (stamp protocol version), `a` (app/workload), `n`
(namespace), `m` (mode), `s` (source), and `e` (sidecar runtime endpoint
locator). Values are percent-encoded. `v`, `a`, `n`, `m`, and `s` are required;
`e` is attached when a target is started and the broker endpoint is known.

The stamp is sidecar's only launch metadata contract. There is no env fallback
for sidecar identity or runtime endpoint data. Business env remains available
through target `env` and `inherits_env`, but sidecar control-plane metadata must
come from `--sidecar-stamp`.

## Inspect Bridge

`sidecar inspect <sidecar> <event> [<json-payload>]` connects to the target's `inspect_socket` and sends one SidecarRuntime event frame. Longer-running provider checks can pass `--inspect-timeout <seconds>`:

- request:  `{"kind":"event","id":"...","verb":"...","payload":<json>}\n`
- response: `{"kind":"event_response","id":"...","payload":<json>}\n`
- error:    `{"kind":"event_error","id":"...","error":{"code":"...","message":"..."}}\n`

When `<json-payload>` is omitted, `sidecar inspect` sends `{}`. This maps
naturally to typed project protocols with unit/no-input events.

Unix sockets are the canonical transport (`unix:///absolute/path.sock`). TCP (`tcp://host:port`) is reserved for non-Unix fallback or explicit compatibility probes.

## Project Protocols

For project-owned typed protocols, the recommended path today is per-workload
`inspect_socket` plus typed event names owned by the project:

```sh
sidecar inspect server server.status
sidecar inspect client client.status
```

`sidecar` owns the transport envelope and timeout. The project owns event names,
payload schemas, response schemas, and any aggregation CLI or crate.
`sidecar status` intentionally stays process-level: it reports sidecar-known pids,
broker state, and namespace identity, not product health or typed protocol
state.

The stamp's `e` field is sidecar runtime infrastructure, not a business
endpoint and not yet a stable public API for project protocol adapters. Public
reusable crates for stamps, inspect envelopes, endpoint parsing, or typed
bootstrap helpers should wait until real projects converge on shared needs.

Report parser gaps, diagnostics noise, install issues, and missing capabilities at:

https://github.com/PerishCode/sidecar/issues
