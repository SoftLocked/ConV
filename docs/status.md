# Project status

The source is published at [SoftLocked/ConV](https://github.com/SoftLocked/ConV)
for cloning and local use.
Public hosting and a new demo recording are outside the current release scope.
The application is a research prototype: local functionality is exercised, and
large-room/load failures remain documented rather than treated as supported
capacity. This status supersedes the original hosted-demo milestone.

| Proposal milestone | Implemented evidence | Remaining gate |
| --- | --- | --- |
| 1. CRDT specification and core | [Specification](crdt-spec.md), independent reference model, merge-algebra and 100-seed differential checks, Unicode/dependency validation | None for the current cooperative scope |
| 2. Replication and membership | Exact causal holes/ranges, snapshots, bounded framing, SWIM, virtual loss/duplication/partition/rejoin scenarios | WAN timer tuning is unverified |
| 3. Local browser editor | WASM, CodeMirror, strict IndexedDB transactions, session-isolated snapshots, import/export/backup, cached offline shell | Real OS IME and physical-device storage checks |
| 4. Browser collaboration | Small full meshes and a 50-session bounded overlay, stable cursor anchors, simultaneous/offline edits, selected-candidate forced TURN, signaling outage, refresh/rejoin | Larger mixed-edit scenarios still miss deadlines; physical-network/background tests are unverified |
| 5. Native scale and measurements | Real UDP sockets at 10/50/100/200 peers, multiple seeds, byte baseline, 1/100/1024 KiB histories, CPU/RSS sidecars, native/WASM merge and 1,000-trial reconnect reports | Multi-host measurements are not established |
| 6. Package and publish | Published GitHub repository, clone-and-run README, pinned toolchains/lockfiles, MIT license, verified local startup, CI workflow and experiment reports | Check the latest [GitHub Actions result](https://github.com/SoftLocked/ConV/actions/workflows/check.yml); hosting is optional |

See [measured results](../benchmarks/reports/current/report.md),
[measurement methods](benchmark-methodology.md), [architecture](architecture.md),
[limits](limitations.md) and [manual release checks](manual-validation.md).

The browser fast path uses source broadcast within small full meshes and
bounded reliable flooding above eight peers, up to a 50-session room. Native
scale experiments retain bounded two-neighbor forwarding. Whole-message retries
and retained-range repair implement incomplete-transfer recovery; targeted chunk
requests and delta piggybacking on probes are optional optimizations not used by
this implementation. The shared control channel prioritizes membership/repair.

The all-200-writer native workload is separate from the original ten-writer scale
measurement and initially failed its settling deadline in all nine cases. These
raw failures and the 50-browser large-note burst failure are retained. The
overlay is a subsequent browser-scale milestone; initial eight-context latency
reports remain measurements of their recorded commits, not 50-person WAN claims.

## Publication verification

The local publication check passed 44 Rust tests, formatting, Clippy, the native
two-process restart/repair smoke test, the topology test, a production build and
16 browser tests including forced loopback TURN. The explicit 50-browser and
performance cases were skipped. A clean checkout passed frozen dependency
installation and `pnpm dev` on separate ports; two isolated browser contexts
exchanged edits and recovered saved text after reload. GitHub Actions reports
remote results independently through the link above.
