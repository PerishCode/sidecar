# sidecar

IPC-based sidecars project manager. Stamp args + inspect bridge over Unix sockets — a small, product-agnostic CLI for managing the lifecycle of multiple sidecar processes for one project.

`sidecar` is intentionally product-agnostic. A consumer (such as `stim.io`) provides an explicit config file; the CLI turns that into validated development plans, lifecycle execution, and an inspect IPC channel.

## Install

Release installation is R2-backed.

```sh
curl -fsSL "$SIDECAR_RELEASES_PUBLIC_URL/stable/latest/install.sh" \
  | sh -s -- install --channel stable --public-url "$SIDECAR_RELEASES_PUBLIC_URL"
```

Beta releases use the same installer with `--channel beta`.

## Local Smoke

After cloning, initialize the local checkout:

```sh
python3 scripts/init.py
```

Run the fast local smoke path:

```sh
cargo run --locked -p cli -- doctor --config examples/minimal.toml
cargo run --locked -p cli -- plan   --config examples/minimal.toml --format json
```

## Release

Stable releases are started from the `release-stable` workflow (`.github/workflows/release.yml`). The workflow resolves the Cargo version against R2 metadata, runs verification, publishes artifacts and installers to R2, then creates the git tag after publish succeeds.

Beta releases are started from `release-beta`. The workflow advances `vX.Y.Z-beta.N` from R2 beta metadata unless a version override is provided.

## Boundary

- `crates/core` owns `Manifest` config, diagnostics, plan generation, socket parsing, stamp args protocol, process discovery, and the inspect IPC client.
- `crates/cli` owns the installed `sidecar` command surface (lifecycle + inspect).
- Consumers own product-specific manifest files and the actual inspect server implementations on their sidecars.

## Stamp args protocol

A consumer that uses `sidecar` to manage a process must accept (and ignore) the canonical stamp args appended to its command line:

```
--sidecar-stamp-app=<sidecar.name>
--sidecar-stamp-namespace=<project.namespace>
--sidecar-stamp-mode=<sidecar.mode>
--sidecar-stamp-source=tool:sidecar
```

These let `sidecar` discover, status-check, and stop running sidecars cross-platform.

## Inspect bridge

`sidecar inspect <sidecar> <event> [<json-payload>]` connects to the sidecar's `inspect_socket` and exchanges one line of JSON:

- request:  `{"event":"...","payload":<json>}\n`
- response: `{"ok":true,"data":<json>}\n` or `{"ok":false,"error":"..."}\n`

Unix sockets are the canonical transport (`unix:///absolute/path.sock`). TCP (`tcp://host:port`) is reserved for non-Unix fallback or explicit compatibility probes.

Report parser gaps, diagnostics noise, install issues, and missing capabilities at:

https://github.com/PerishCode/sidecar/issues
