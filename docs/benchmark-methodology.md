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
counts. Reports separately divide combined sent/received application bytes by the
active edit window and the whole edit-plus-settle duration. Short burst experiments must not be
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
reported independently. Runs declare their warmup and host-randomness seed. These protocol timings are distinct from isolated document-merge timings below.
Short network runs and limited repetitions make their percentiles exploratory.

Reports contain source commit, dirty flag, toolchain, CPU model, execution
model, workload, duration, byte counts, merge samples and process memory.
`scripts/run-experiments.py` runs serial workloads and writes GNU-time sidecars
with user/system CPU seconds, elapsed seconds and peak RSS for the entire child
process, including preparation and reporting. Whole-process CPU time divided by
elapsed time gives average CPU cores consumed; it is not an active-window
protocol-only utilisation measurement. Initial reports without sidecars did not
collect CPU time. The current suite includes repeated seeds, deletion-heavy and
idle workloads and 1,000 measured recovery trials. Raw data and published
interpretation are under benchmarks/.

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

## Reproduction and interpretation

Build release binaries with `cargo build --release -p conv-native-peer --bins`,
then run `python3 scripts/run-experiments.py`. The timestamped result directory
contains a manifest, every raw JSON report and matching `.resources.json`
sidecars. `--quick` is useful for smoke checks but never qualifies recovery p99.
Do not run browser tests, compilation or other heavy workloads concurrently.

Run `python3 scripts/report-experiments.py --directory PATH` to generate Markdown
and standalone SVG/PNG figures. Matplotlib is required. The renderer refuses
dirty/failed results and mismatched byte-baseline pairs. It uses nearest-rank
percentiles (`ceil(n*p)-1` as a zero-based index). Earlier 30-sample browser raw
reports retain a floor-index p95 field; their distributions are preserved and
the renderer recomputes the correct statistic from samples. The current browser
measurement uses 100 sequential visibility samples. These are repeated edit
observations in one room, not 100 independently established networks.

Document-size runs use ten replicas at 100 KiB and two at 1 MiB to bound this
host's memory consumption; 200-peer results use 1 KiB initial visible text. They
do not establish 200-peer scaling at every document size. Large-paste correctness
is tested with 8,200 emoji through the editor and backup restore.

Native scale writers start on phase-aligned interval timers, a deliberate burst
profile. Multiple seeds vary shuffled peer selection; they do not independently
resample hardware or network conditions. The local browser rendering metric is
a second-animation-frame proxy. Browser CPU/memory contention is not isolated
per participant, and no physical display or WAN latency is inferred.
