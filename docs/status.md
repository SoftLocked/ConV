# Proposal delivery status

The application and shared engine are implemented and exercised locally. The
hosted release remains pending. This table tracks the proposal's exit criteria,
not an assumption that every stretch performance target must be met.

| Proposal milestone | Implemented evidence | Remaining gate |
| --- | --- | --- |
| 1. CRDT specification and core | [Specification](crdt-spec.md), independent reference model, merge-algebra and 100-seed differential checks, Unicode/dependency validation | None for the current cooperative scope |
| 2. Replication and membership | Exact causal holes/ranges, snapshots, bounded framing, SWIM, virtual loss/duplication/partition/rejoin scenarios | WAN timer tuning is unverified |
| 3. Local browser editor | WASM, CodeMirror, strict IndexedDB transactions, session-isolated snapshots, import/export/backup, cached offline shell | Real OS IME and physical-device storage checks |
| 4. Browser collaboration | Eight-context mesh, stable cursor anchors, simultaneous/offline edits, selected-candidate forced TURN, signaling outage, refresh/rejoin | Separate physical networks, background/suspended tabs |
| 5. Native scale and measurements | Real UDP sockets at 10/50/100/200 peers, multiple seeds, byte baseline, 1/100/1024 KiB histories, CPU/RSS sidecars, native/WASM merge and 1,000-trial reconnect reports | Multi-host measurements are not established |
| 6. Package and publish | Fresh local clone with frozen/offline install, one-command startup, editing and reload verified; Docker Compose/Caddy/Coturn configuration, CI workflow, reports and demo scripts | Container execution, public host/domain, live deployment and physical-device demo |

See [measured results](../benchmarks/reports/current/report.md),
[measurement methods](benchmark-methodology.md), [architecture](architecture.md),
[limits](limitations.md) and [manual release checks](manual-validation.md).

The browser fast path intentionally uses source broadcast within its eight-peer
full mesh, following measured visibility tails with two-neighbor gossip. Native
scale experiments retain bounded two-neighbor forwarding. Whole-message retries
and retained-range repair implement incomplete-transfer recovery; targeted chunk
requests and delta piggybacking on probes are optional optimizations not used by
this implementation. The shared control channel prioritizes membership/repair.
