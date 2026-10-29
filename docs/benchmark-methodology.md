# Experiments

`conv-bench` runs one Tokio task per peer, with a separate real UDP socket on
IPv4 loopback. It is not a multi-host deployment. Membership, framing, document
merge, history and anti-entropy use the same crates as browser peers.

Each peer begins with identical preloaded history for the declared document size;
bootstrap is outside the measurement window by default. A readiness barrier
prevents document initialization from consuming the edit window. The default
one-second warmup is excluded from byte and merge measurements. Active writers insert one scalar
per mutation, at a declared interval. After writers stop, a declared settling
period allows repair. State and causal summaries must match for every peer or
the command fails. The raw report records convergence even on failure.

All sent and received application frame bytes, including control and repair,
are counted. IPv4/UDP header bytes are recorded separately using actual datagram
counts. Per-node rates divide combined sent/received application bytes by the
whole measured edit-plus-settle duration. Short burst experiments must not be
presented as steady-state rates. Sender-side bootstrap preparation is excluded.

`--full-state` is restricted to two peers so snapshot receipt and delta receipt
have identical forwarding topology. It replaces each outgoing mutation/batch message with the sender's complete
retained CRDT mutation history snapshot at that same send event, preserving
batching, neighbor topology, UDP settings, control messages and repair. This is
a deliberately simple complete-state baseline. It is not a comparison with
Yjs/Automerge. Compare only matching workloads, state sizes, actual mutation
counts, and converged runs. `--network-bootstrap --warmup-seconds 0` initializes history only on peer 1
and includes bootstrap in whole-session byte counts. Report this separately
from the preloaded steady-operation baseline.

Merge timings include protocol bookkeeping around a decoded mutation, excluding
network, decoding, rendering and backup writes. Native and WASM timings must be
reported independently. Runs declare their warmup and host-randomness seed. Short runs and limited
repetitions still make their percentiles exploratory. They do not establish the proposal's 100 KiB native/WASM p99 target.

Reports contain source commit, dirty flag, toolchain, CPU, execution model,
workload, duration, byte counts, merge samples and process memory. CPU-time
profiling, multiple repeated seeds/warmups, deletion-heavy/idle workloads,
1,000-trial reconnect distributions, and browser key-to-paint measurements remain
required before publishing the corresponding performance claims. Proposed
numbers stay pending until those measurement contracts are satisfied.

## Reconnect trials

`conv-recovery` prepares 5,000 single-scalar mutations on one offline replica.
Eight peers use separate real UDP sockets and the same snapshot/range protocol.
A two-phase readiness barrier excludes preparation and starts the timer just
before restored bidirectional reachability is released. Writers remain stopped.
The timer finishes only when every target has identical full document state and
causal summary. Membership probes remain active. Trials use fresh sockets and
room IDs to prevent prior datagrams from affecting a later trial.

The default command runs ten warmups and 1,000 measured trials. Raw durations
and failure counts are retained. A qualified p99 is emitted only after at least
1,000 successful measured trials and no measured failures. Smaller smoke tests
explicitly leave that field null. This is a native loopback result, not a WAN or
browser recovery guarantee.

## Isolated merge and foreground browser measurements

`conv-merge` and the WASM `MergeExperiment` use the same deterministic
`MergeWorkload`. Both build the initial document before timing, warm up with
small live deltas, time single-scalar incremental merges, and verify final
canonical state. Native measurements use Instant; WASM uses host performance.now
around a WASM call and reports its observed clock resolution. Browser results
include a conservative upper bound at that resolution. Serialization, rendering,
network and persistence are outside these isolated merge intervals.

`CONV_BENCH_BROWSER=1 pnpm test:browser --grep 'measure isolated'` additionally
records trusted-keydown-to-second-animation-frame timing on a 1 KiB document,
and controller-observed visibility across eight independent Chromium contexts.
The former is a rendering proxy, not a physical screen measurement; the latter
uses one controller clock and includes automation overhead. Neither result is a
WAN claim. Native/WASM timings remain separate.

`--delete-every N` changes the live network workload to delete a previously
inserted local scalar when scheduled. Reports retain tombstone counts. `--writers
0` runs an idle-room workload. Native isolated merge workloads also support a
deletion-heavy initial history through `--deletion-heavy`.

Reports separate active-edit-window bandwidth from combined edit-plus-settle
bandwidth. Use active-window rates when discussing steady writing; report
recovery/settling bytes as well. Qualified steady-state conclusions require
longer runs than these initial smoke/scale experiments.
