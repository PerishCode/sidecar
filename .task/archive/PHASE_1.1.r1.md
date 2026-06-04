# Phase: 1.1 - Capture TCP Broker Runtime Design

## Objective
Record the agreed architecture and implementation boundaries for moving sidecar IPC toward a project/namespace-scoped local TCP broker, so a follow-up implementation session can start without replaying the design discussion.

## Exit Criteria
- [x] Capture the intended broker lifecycle and identity model.
- [x] Capture the no-file-persistence discovery principle.
- [x] Capture platform-specific listener discovery strategy, including Windows.
- [x] Capture first implementation slice and known deferrals.

## Work Log
- [2026-06-03T15:12:59Z] STARTED: Phase initialized.
- [2026-06-03T15:12:59Z] Documented the broker design as a temporary planning artifact under `.task/`.

## Technical Notes
- **Files Touched:** `.task/MAIN.md`, `.task/PHASE_1.1.md`
- **New Dependencies:** None yet. Future Windows listener discovery likely needs `windows-sys`.
- **Blockers:** None for planning. Implementation still needs detailed API cuts.

## Design Summary

`sidecar` should evolve from a target-local one-shot inspect bridge toward a local TCP runtime control plane. The broker is not global: each broker serves exactly one project and one namespace. The broker is created by `sidecar start` and should end when `sidecar stop` or `sidecar reset` destroys all managed processes in that namespace.

The broker is best understood as a project/namespace-scoped runtime endpoint for a local multi-process instance. It is not a persistent daemon, service mesh, or deployment system. It exists only to give the process group a predictable local control/debug surface.

## Core Principles

- Use TCP loopback as the primary runtime IPC substrate for cross-platform consistency.
- Keep one broker/listener per `project * namespace`.
- Do not write broker endpoint/bind information to files.
- Do not place probeable runtime information, such as bind address or endpoint, into argv metadata.
- Use argv metadata only for identity and isolation data that would otherwise need file coordination.
- Treat the running process table and each process' argv as the source of truth for identity.
- Treat open TCP listeners and protocol handshake as the source of truth for runtime capability.

## Broker Identity

Introduce a broker-specific argv marker instead of overloading target stamp identity:

```text
--sidecar-broker=p=<project>;n=<namespace>;s=tool%3Asidecar
```

Short keys:

- `p`: project name
- `n`: namespace
- `s`: source

Do not include bind, endpoint, pid, readiness, target registry, capabilities, or health fields in `--sidecar-broker`. Those are runtime/probeable facts, not isolation identity.

Target processes continue to use packed target stamp:

```text
--sidecar-stamp=a=<target>;n=<namespace>;m=<mode>;s=tool%3Asidecar
```

## Endpoint Discovery

Discovery should be:

1. Find broker process by `--sidecar-broker` argv identity.
2. For the broker pid, discover current TCP LISTEN sockets from the OS.
3. Probe loopback listener candidates.
4. Confirm with a broker handshake that the listener serves the expected project and namespace.

This avoids:

- state files for broker endpoint persistence
- endpoint fields in argv
- deterministic port windows
- port collision handling in state
- stale file consistency problems

## Platform Listener Discovery

Expose an internal core API shaped roughly like:

```rust
pub fn tcp_listeners_for_pid(pid: u32) -> Result<Vec<SocketAddr>, String>
```

Platform plan:

- Linux: parse `/proc/<pid>/fd/*` socket inodes and join against `/proc/net/tcp` and `/proc/net/tcp6`.
- macOS: first implementation can shell out to `lsof -Pan -p <pid> -iTCP -sTCP:LISTEN`, with parser tests. Native API can come later.
- Windows: use IP Helper API through `windows-sys`, specifically `GetExtendedTcpTable` with owner-pid listener tables.

Windows details:

- Use `GetExtendedTcpTable`.
- Query IPv4 with `AF_INET` and `TCP_TABLE_OWNER_PID_LISTENER`.
- Query IPv6 with the corresponding IPv6 owner-pid listener table.
- Filter rows by `dwOwningPid == broker_pid`.
- Keep rows in LISTEN state.
- Convert address and port byte order carefully.
- Prefer this native API over `netstat`, PowerShell, `netstat2`, or `listeners` for core discovery reliability.

Potential dependency:

```toml
[target.'cfg(windows)'.dependencies]
windows-sys = { version = "...", features = [
  "Win32_Foundation",
  "Win32_NetworkManagement_IpHelper",
  "Win32_Networking_WinSock",
] }
```

## Broker Protocol: First Minimum

Start with a minimal line-delimited JSON handshake.

Client:

```json
{"kind":"hello","protocol":1,"project":"<project>","namespace":"<namespace>"}
```

Server:

```json
{"kind":"hello_ok","protocol":1,"project":"<project>","namespace":"<namespace>"}
```

Connections that do not match project/namespace/protocol should close. This protects probe logic from misidentifying unrelated loopback listeners.

## Lifecycle

`sidecar start`:

1. Resolve project and namespace.
2. Discover existing broker by `--sidecar-broker`.
3. Probe and handshake to confirm it is usable.
4. If absent or stale, spawn internal broker process.
5. Wait for broker readiness by pid listener discovery + handshake.
6. Start manifest targets.
7. Inject runtime endpoint into targets:

```text
SIDECAR_RUNTIME_ENDPOINT=tcp://127.0.0.1:<port>
```

`sidecar stop`:

- Stop selected manifest targets.
- For full stop, terminate broker.
- For `stop <target>`, terminate broker only when no other manifest target in that namespace is still running.
- Do not count the broker itself as a remaining target.

`sidecar reset`:

- Terminate all stamped target processes for the namespace.
- Terminate all broker-flagged processes for the namespace.
- Existing project data cleanup remains separate; broker endpoint is not persisted there.

`sidecar status/list`:

- Discover targets by `--sidecar-stamp`.
- Discover broker by `--sidecar-broker`.
- Resolve runtime endpoint by pid listener discovery + handshake.
- Surface runtime health/endpoint in output as live probe data, not persisted state.

## First Implementation Slice

Recommended first PR scope:

1. Add broker flag codec, separate from target stamp codec.
2. Add TCP listener discovery module with Linux/macOS/Windows implementations.
3. Add internal `sidecar runtime serve` command, hidden from normal help.
4. Implement minimal broker TCP server and handshake.
5. Make `start` ensure broker exists and inject `SIDECAR_RUNTIME_ENDPOINT`.
6. Make `status/list` show broker runtime endpoint when discovered.
7. Make `stop/reset` terminate broker processes.
8. Keep existing target pid/log state for now.
9. Do not migrate inspect forwarding yet.

## Explicit Deferrals

- Do not move `inspect <target> <event>` onto broker in the first slice.
- Do not remove target pid/log state in the first slice.
- Do not persist broker endpoint to `runtime.json`, `targets.json`, or any other file.
- Do not place bind/endpoint in `--sidecar-broker`.
- Do not add a global daemon or global registry port.

## Release / Compatibility

This is a runtime architecture change and should bump the CLI/core minor version, likely `0.4.0`.

Even if the first slice keeps old inspect behavior, the lifecycle/env surface changes. PR descriptions should include `## Compatibility` and note that the broker is discovered by process argv plus live TCP listener probing, not by persisted files.

---
*This phase will be popped/archived upon meeting exit criteria.*
