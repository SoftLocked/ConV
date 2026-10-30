# Product and measurement limits

ConV has no central document authority or cloud document storage. A reachable
replica must retain the document for a newcomer to obtain it. Browser storage
can be evicted or cleared; export backups before relying on a device indefinitely.
Unsaved edits can be lost on a crash. A peer acknowledgement is not durable backup.

Room links are bearer invitations. Anyone with a link may read and edit. WebRTC
protects transport links, but independently authenticated end-to-end encryption
and protection from malicious signaling are outside the initial guarantees.
The native UDP adapter is an unencrypted cooperative lab transport.

The browser room is capped at 50 sessions, with at most eight direct links per
session (six steady overlay links above eight participants). Browser and real-device results must
be distinguished from native task/socket experiments. Native 200-peer results do
not validate a 200-connection browser overlay. Background tabs and mobile network
changes require physical-device validation. OS-level IME behavior requires manual
checks in addition to Chromium composition tests.

Tombstones and mutation history are retained. Safe garbage collection with
indefinitely offline replicas is not implemented. Per-session browser snapshots
also accumulate intentionally to preserve offline tabs. Capacity limits include
8 MiB framed messages, eight incomplete transfers with ten-second expiry,
65,536 records/targets per mutation, 100,000 mutations per bootstrap snapshot,
1,000,000 mutations/64 MiB per native backup, 65,536 historical writer sessions,
and 100,000 pending predecessor dependencies. Oversized snapshots fall back to
range repair. Exceeding local capacity requires exporting work and moving it to
a new document or a larger-capacity future adapter; existing history is not
silently discarded. Binary browser backups add a small metadata header.

Rich text, accounts, permissions, comments, attachments, code execution, AI,
Git integration, and collaborative undo remain excluded by design. Generic undo
shortcuts are disabled rather than reverting collaborators' text. Imported text
opens a new room; CRDT backups preserve identity and history.

Performance targets are measurements to investigate. Reports identify workload,
transport, execution model and measurement scope. Do not describe loopback as
WAN, animation-frame timing as physical-screen timing, or steady-state byte
reduction as a whole-session reduction. Do not claim minimal cost or superiority
to established CRDT libraries from the deliberately simple full-state baseline.

Public hosting is outside the current clone-and-run release scope. Docker
execution, cross-network physical devices and background-tab/resume behavior
remain unverified; local publication does not establish those capabilities.

The initial 200-native-peer measurements used ten writers. All-200-writer stress
subsequently failed a declared 30-second settling deadline across three seeds
and three workloads; retained failures are under benchmarks/stress-results/.
The 50-browser integration test demonstrates bounded links and complete-state
recovery on loopback. A large simultaneous review-note burst exposed additional
backlog under memory pressure. Neither result establishes sustained high-rate
50-browser service, a 200-browser room, or a long soak.
