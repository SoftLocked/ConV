# ConV measured experiments

These are local laboratory measurements. Native peers are Tokio tasks in one process using independent IPv4 UDP sockets on loopback. Browser tests use independent Chromium contexts on the same host. No result is a WAN or multi-host claim.

The active writers use phase-aligned interval timers after a common readiness barrier and declared warmup. Rate results include sent and received application frame bytes, control and repair; IPv4/UDP bytes are separate in the raw reports. Active-window rates and edit-plus-settle rates are distinct.

| Native peers | Mean active KB/s/node | p95 active KB/s/node | Mean whole-window KB/s/node | Process peak MiB | Mean CPU cores | Commit |
| ---: | ---: | ---: | ---: | ---: | ---: | --- |
| 10 | 15.43 | 15.84 | 7.52 | 29.5 | 0.027 | `1cfc2ca` |
| 50 | 16.87 | 17.82 | 8.21 | 58.0 | 0.115 | `1cfc2ca` |
| 100 | 16.93 | 17.45 | 8.25 | 114.5 | 0.212 | `1cfc2ca` |
| 200 | 16.96 | 17.61 | 8.27 | 219.5 | 0.370 | `1cfc2ca` |

Raw source directory: `benchmarks/raw-results/20261029T224234Z`.

All listed replicas converged with identical document state and causal receipt. Short runs are exploratory; active writers, edit cadence, warmup, settling time and actual mutation counts are retained in each raw report.

| Initial visible text | Peers | Actual mutations per replica | Serialized history bytes/node | Tombstones/node |
| --- | ---: | ---: | ---: | ---: |
| 1 KiB | 10 | 251 | 51,678 | 0 |
| 100 KiB | 10 | 175 | 4,911,034 | 0 |
| 1024 KiB | 2 | 271 | 51,857,470 | 0 |

## Bandwidth baseline

The matched **two-peer** comparison reduced combined sent/received application bytes by **99.06%** with preloaded state and **95.25%** in the short whole-session run including network bootstrap. These are separate claims. The whole-session result does not establish a universal 95% reduction.

The retained initial three-second whole-session comparison saved 93.27%; the profiled five-second comparison has more live edits over which to amortize bootstrap. Both raw pairs remain available. Savings depend on session length and history size.

The baseline sends complete retained CRDT history at the same batching cadence/topology and UDP settings. This is a simple full-state baseline, not a comparison with established CRDT libraries.

## Isolated merge

| Native 100 KiB seed | Warmup / measured | p99 microseconds | Verified |
| ---: | --- | ---: | --- |
| 0 | 200 / 2000 | 2.42 | True |
| 1 | 200 / 2000 | 2.42 | True |
| 2 | 200 / 2000 | 2.52 | True |

## Reconnect recovery

1,000 measured trials after 10 warmups; 0 measured failures. Eight native UDP peers recovered 5,000 retained single-scalar mutations with a qualified p99 of **163.97 ms**. Writers were stopped, and the timer ended only when all targets had identical state and causal summaries. This is loopback recovery after restored reachability, excluding peer setup and document generation.

## Browser and WASM

Chromium 156.0.8078.4, Intel(R) Core(TM) i9-9900K CPU @ 3.60GHz; commit `b8af802`. Across three seeds on a 100 KiB document, the isolated WASM merge p99 clock-resolution upper bound was **0.200 ms**. Rendering, network, serialization and protocol history are excluded; the WASM boundary is included.

The 1 KiB trusted-keydown-to-second-animation-frame proxy had p95 **30.40 ms**. This is not physical-screen timing. Controller-observed visibility at all seven remote DOMs in an eight-context foreground room had p95 **130.37 ms**, including automation overhead, with one writer waiting for each edit to become visible before sending the next.

## Browser dissemination comparison

The retained 30-edit fanout-two browser run had nearest-rank p95 visibility of 863.57 ms. Source broadcast within the bounded eight-peer mesh measured 188.16 ms in the next 30-edit run and 130.37 ms in the final 100-edit run. These are separate single-host runs, not a controlled WAN comparison. Earlier raw p95 fields used floor indexing; these figures are recomputed from the retained samples.

## Interpretation

The roughly 15 KB/s target is assessed using active-window traffic, rather than diluting it with quiet settling time. Keep exact measured values and workload phases in any resume/demo statement. Short trials and a single host limit generality. Physical-device composition/background tabs, WAN behavior and container execution remain unverified. Public hosting is outside the current local release scope.

GNU-time resource sidecars, when present, record whole-process user/system CPU seconds and peak RSS, including preparation and report generation. They are distinct from the protocol measurement window. CPU utilisation was not collected for the initial reports lacking those sidecars.
