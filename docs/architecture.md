# ConV architecture

The Rust document crate implements a tombstone sequence CRDT with stable element
IDs, deterministic anchored traversal, join validation, Unicode scalar elements,
and UTF-16/cursor adapters. An indexed dependency path is checked against a
separate clarity-first reference model. Writer IDs are fresh per session;
mutation counters and ordering clocks are separate.

The protocol crate retains immutable transactions, tracks exact causal receipt
including holes, and repairs missing ranges. Native gossip forwards a new mutation once per replica to at most two neighbors
selected from host-shuffled traversals. Small browser rooms broadcast directly
from the source. Rooms above eight use a bounded six-neighbor reliable overlay
and forward each newly learned mutation once; see browser-overlay.md.
Summary exchanges are bidirectional. Hosts batch small deltas on a 20 ms cadence;
control bypasses bulk queues. Snapshots merge atomically with newer local work.
Postcard frames carry version, room, sender, message ID, chunk positions and CRC.
Bounded reassembly and repeated anti-entropy repair incomplete messages.

SWIM uses direct and indirect probes, suspicion, incarnation refutation and
ordered membership observations. Membership and document history are separate;
a failed/suspect/left observation cannot erase edits. The host supplies time,
randomness, transport and persistence; the core imports no Tokio, browser API,
wall clock or disk operation.

The native adapter supplies Tokio UDP and atomically replaced, fsynced backup
files. Benchmarks use independent sockets and Tokio tasks in one process; those
results are explicitly loopback experiments. The deterministic simulator uses
the same encoded messages and virtual timers with seeded failure injection.

The browser adapter shares the same protocol and document crates through WASM.
A synchronous Rust replica translates CodeMirror transactions. A worker replica
owns protocol scheduling and per-session IndexedDB persistence. Exactly one
replica allocates local mutation IDs; both merge identical mutation messages.
See browser-architecture.md for ordering, IME and durability details.

WebRTC data channels carry documents; a separate unreliable control channel
prioritizes probes and repair metadata, while ephemeral presence has its own
channel. Signaling relays only validated SDP/ICE metadata and room membership
hints. A TURN relay may carry encrypted WebRTC traffic. HTTP hosting/signaling
and relays remain necessary infrastructure; ConV is not serverless.
