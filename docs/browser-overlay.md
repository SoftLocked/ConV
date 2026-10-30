# Browser connection overlay

Rooms admit up to 50 browser sessions. Up to eight members use a full mesh with
source-only document broadcast. Larger rooms use a ring ordered by server-issued
join order, with links at offsets -4, -2, -1, +1, +2 and +4. Each member normally
has six direct WebRTC connections. Two temporary spare slots permit new links
to open before old links retire; the hard per-session limit is eight.

The server supplies ephemeral roster/order hints and relays SDP/ICE. It never
receives document traffic. Document messages travel over reliable ordered peer
channels. In an overlay, each newly accepted mutation is forwarded once to all
other adjacent links; immutable mutation IDs suppress loops. Exact summaries and
range repair recover missed fast-path sends. Large-room SWIM uses a four-second probe period, one-second direct/indirect
timeouts and twenty-second suspicion, allowing multi-hop scheduling.
The native UDP gossip policy
remains separate and retains two-neighbor forwarding.

Membership probes use the full known room roster. Control messages addressed to
non-neighbors use greedy ring routing over peer control channels, with a
16-hop limit and bounded deduplication. Small probes remain unordered and
unreliable; fragmented summaries use the reliable document channel. Presence
carries its origin and sequence, expires in the UI, and floods with the same hop
limit. Per-link queues coalesce cursors by origin. At most five recently active cursors are rendered in larger rooms
(twelve in small rooms), with unchanged decorations left intact.

On join, deterministic offer initiation avoids duplicate offers. Topology
updates are debounced, old links retire after replacement links open, and ICE
repair refreshes relay credentials. Metadata loss alone preserves healthy
channels. A session that has needed overlay forwarding keeps that mode during
roster shrinkage, including a signaling outage. No roster count or neighbor
acknowledgement claims every device is durably synchronized.

The worker coalesces durable snapshots on a 100 ms cadence, with one transaction
in flight. Edits continue integrating while a snapshot commits. Only the latest
included local serial is acknowledged after a strict IndexedDB transaction, and
newer changes require a subsequent save. A quota/write failure remains visible;
unsaved changes are still at risk on a crash. The passive protocol worker avoids
rendering text on timer/control events. Reliable document frames stream into the editor replica’s independent
reassembler; repeated full-history UI snapshots are avoided. Text rendering is
cached until document history changes. A backup merge repairs an editor-side
reassembly error or an IME composition gap, preserving newer local edits.

Local editor frames have a separate bounded reassembler, so incomplete network
transfers cannot reject an otherwise valid local edit. Complete single-frame
messages require no incomplete-transfer slots. Incomplete remote messages still
have bounded count, bytes and expiry; anti-entropy retries rejected remote data.

## Verification and limits

The topology test covers symmetric links, bounded degree, connectivity and
greedy reachability for every size from two through 50, plus recomputed graphs
after departures. Browser integration exercises multiple hops, contributions
from every session, offline/rejoin and byte-identical complete CRDT backups.
The optional 50-session test also checks capacity and a signaling outage.

```sh
node --test tests/network/topology.test.mjs
CONV_STRESS_BROWSER=1 pnpm --dir web test:browser --grep '50 browser replicas'
```

Fifty isolated Chromium profiles can exhaust the memory of a small load-generator
host. The local machine has about 8 GiB available to WSL and uses swap during this
run. These results establish local integration, not WAN latency, throughput or a
long soak. Background/suspended devices, repeated churn, TURN under large-room
load and multi-host deployments remain distinct validation gates. Bounded
routing can drop control/presence under congestion; timers retry control, and
presence is ephemeral. Room links retain the cooperative bearer-invitation
security model described in limitations.md.
