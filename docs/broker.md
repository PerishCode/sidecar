# TCP Broker Runtime Design

This document captures the next runtime direction for `sidecar` after the
operator/release surface has been aligned with `runseal`, `flavor`, and the
root `manage.sh|ps1` managers.

The broker is a local TCP runtime control plane scoped to one project and one
namespace. It is created by `sidecar start` and should disappear when
`sidecar stop` or `sidecar reset` tears down the managed process group for that
namespace. It is not a global daemon, a deployment system, or a persistent
service registry.

## Principles

- Use loopback TCP as the runtime IPC substrate for cross-platform consistency.
- Keep one broker/listener per `project * namespace`.
- Do not persist broker endpoint or bind information to files.
- Do not put probeable runtime data, such as bind address or endpoint, in argv.
- Use argv metadata only for identity and isolation facts that would otherwise
  need file coordination.
- Treat the process table and argv as the identity source of truth.
- Treat open TCP listeners plus a protocol handshake as the runtime capability
  source of truth.

## Broker Identity

Broker processes use a broker-specific packed argv marker:

```text
--sidecar-broker=p=<project>;n=<namespace>;s=tool%3Asidecar
```

Fields:

- `p`: project name.
- `n`: namespace.
- `s`: source.

The broker marker must not carry bind, endpoint, pid, readiness, target
registry, capability, or health data. Those are live runtime facts and must be
discovered by probing.

Managed target processes continue to use the target stamp:

```text
--sidecar-stamp=a=<target>;n=<namespace>;m=<mode>;s=tool%3Asidecar
```

## Endpoint Discovery

Discovery should resolve a broker endpoint without any persisted endpoint
state:

1. Find the broker process by `--sidecar-broker` argv identity.
2. Discover current TCP `LISTEN` sockets owned by that broker pid.
3. Probe loopback listener candidates.
4. Confirm the listener with a broker handshake for the expected project and
   namespace.

This avoids stale endpoint files, deterministic port windows, port collision
state, and bind information in argv.

## Listener Discovery

Core should expose an internal API shaped roughly like:

```rust
pub fn tcp_listeners_for_pid(pid: u32) -> Result<Vec<SocketAddr>, String>
```

Platform strategy:

- Linux: parse `/proc/<pid>/fd/*` socket inodes and join them against
  `/proc/net/tcp` and `/proc/net/tcp6`.
- macOS: first slice may shell out to
  `lsof -Pan -p <pid> -iTCP -sTCP:LISTEN`, with parser tests. A native API can
  replace it later.
- Windows: use IP Helper API through `windows-sys`, specifically
  `GetExtendedTcpTable` with owner-pid listener tables for IPv4 and IPv6.

For Windows, filter rows by owner pid and listener state, then convert address
and port byte order carefully. Prefer this native path over `netstat`,
PowerShell, or third-party listener tools in core discovery.

## Minimum Protocol

Start with a line-delimited JSON handshake.

Client:

```json
{"kind":"hello","protocol":1,"project":"<project>","namespace":"<namespace>"}
```

Server:

```json
{"kind":"hello_ok","protocol":1,"project":"<project>","namespace":"<namespace>"}
```

Connections that do not match the expected project, namespace, or protocol
should close. The handshake prevents probe logic from mistaking unrelated
loopback listeners for a sidecar broker.

## Lifecycle

`sidecar start` should:

1. Resolve project and namespace.
2. Discover an existing broker by `--sidecar-broker`.
3. Probe and handshake to confirm it is usable.
4. Spawn an internal broker process if absent or stale.
5. Wait for broker readiness by pid listener discovery plus handshake.
6. Start manifest targets.
7. Inject the discovered endpoint into targets:

```text
SIDECAR_RUNTIME_ENDPOINT=tcp://127.0.0.1:<port>
```

`sidecar stop` should stop selected manifest targets. A full stop should also
terminate the broker. `stop <target>` should terminate the broker only when no
other manifest target in that namespace remains running; the broker itself does
not count as a remaining target.

`sidecar reset` should terminate all stamped target processes and all
broker-flagged processes for the namespace. Project data cleanup remains
separate, and no broker endpoint state exists to clean up.

`sidecar status` and `sidecar list` should discover target processes by
`--sidecar-stamp`, discover the broker by `--sidecar-broker`, resolve the
runtime endpoint by live listener probing plus handshake, and surface runtime
health as live data.

## Implemented First Slice

The first runtime slice implements:

1. A broker flag codec separate from the target stamp codec.
2. TCP listener discovery with Linux, macOS, and Windows implementations.
3. A hidden internal `sidecar runtime serve <project> <namespace>` command.
4. A minimal broker TCP server and hello handshake.
5. `start` broker ensure plus `SIDECAR_RUNTIME_ENDPOINT` injection.
6. `status` and `list` runtime pids/endpoint output.
7. `stop` and `reset` broker termination.
8. Existing target pid/log state remains in place.
9. Inspect forwarding remains on the current target-local bridge.

## Deferrals

- Do not move `inspect <target> <event>` onto the broker in the first slice.
- Do not remove target pid/log state in the first slice.
- Do not persist broker endpoint to `runtime.json`, `targets.json`, or any
  other file.
- Do not place bind/endpoint data in `--sidecar-broker`.
- Do not add a global daemon or a global registry port.

## Compatibility

This runtime architecture change bumps the CLI/core minor version to `0.4.0`.
Even though the first slice keeps old inspect behavior, lifecycle, environment,
and status/list output behavior changed because the runtime endpoint is
discovered by argv identity plus live TCP probing instead of persisted files.
