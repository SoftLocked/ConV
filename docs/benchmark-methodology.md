# Experiments

`conv-bench` runs one Tokio task per peer, with a separate real UDP socket on
IPv4 loopback. It is not a multi-host deployment. Membership, framing, document
merge, history and anti-entropy use the same crates as browser peers.

Each peer begins with identical preloaded history for the declared document size;
bootstrap is outside the measurement window. Active writers insert one scalar
per mutation, at a declared interval. After writers stop, a declared settling
period allows repair. State and causal summaries must match for every peer or
the command fails. The raw report records convergence even on failure.

All sent and received application frame bytes, including control and repair,
are counted. IPv4/UDP header bytes are recorded separately using actual datagram
counts. Per-node rates divide combined sent/received application bytes by the
whole measured edit-plus-settle duration. Short burst experiments must not be
presented as steady-state rates. Sender-side bootstrap preparation is excluded.

`--full-state` replaces each outgoing mutation message with the sender's complete
retained CRDT mutation history snapshot at that same send event, preserving
batching, neighbor topology, UDP settings, control messages and repair. This is
a deliberately simple complete-state baseline. It is not a comparison with
Yjs/Automerge. Compare only matching workloads, state sizes, actual mutation
counts, and converged runs. Whole-session bandwidth comparisons including network
bootstrap require an additional experiment; these runs report steady operation
with preloaded state.

Merge timings include protocol bookkeeping around a decoded mutation, excluding
network, decoding, rendering and backup writes. Native and WASM timings must be
reported independently. These initial runs have no warmup; their percentiles are
exploratory. They do not establish the proposal's 100 KiB native/WASM p99 target.

Reports contain source commit, dirty flag, toolchain, CPU, execution model,
workload, duration, byte counts, merge samples and process memory. CPU-time
profiling, multiple repeated seeds/warmups, deletion-heavy/idle workloads,
1,000-trial reconnect distributions, and browser key-to-paint measurements remain
required before publishing the corresponding performance claims. Proposed
numbers stay pending until those measurement contracts are satisfied.
