# All-writer native stress results

Each peer is a Tokio task with its own real UDP loopback socket. Every peer writes. These are single-host engine/transport stress checks, not browser or WAN capacity claims.

Requested 200 writers × 5 single-scalar mutations/s for 10 seconds; settling deadline 30 seconds. Minimum accepted offered-load fraction: 95%.

Pass requires identical full CRDT state and causal summaries, every generated mutation retained by every replica, every writer participating, and the configured load/faults actually executing. Failed runs are retained.

| Case / seed | Generated / requested | Load achieved | Complete convergence | Injected loss / partition drops | Peak RSS MiB | Result |
| --- | ---: | ---: | --- | ---: | ---: | --- |
| healthy / 0 | 9980 / 10000 | 99.8% | False | 0 / 0 | 3232.9 | FAIL |
| healthy / 1 | 10000 / 10000 | 100.0% | False | 0 / 0 | 3503.0 | FAIL |
| healthy / 2 | 10000 / 10000 | 100.0% | False | 0 / 0 | 3505.3 | FAIL |
| lossy-partition / 0 | 10000 / 10000 | 100.0% | False | 36075 / 63083 | 3057.9 | FAIL |
| lossy-partition / 1 | 10000 / 10000 | 100.0% | False | 36061 / 63022 | 3062.5 | FAIL |
| lossy-partition / 2 | 10000 / 10000 | 100.0% | False | 36220 / 64998 | 3068.9 | FAIL |
| deletion-heavy / 0 | 10000 / 10000 | 100.0% | False | 0 / 0 | 2805.5 | FAIL |
| deletion-heavy / 1 | 10000 / 10000 | 100.0% | False | 0 / 0 | 2801.5 | FAIL |
| deletion-heavy / 2 | 10000 / 10000 | 100.0% | False | 0 / 0 | 2791.4 | FAIL |

## Failures

- Failure: process exit 1; canonical state, complete history or writer participation failed
- Failure: process exit 1; canonical state, complete history or writer participation failed
- Failure: process exit 1; canonical state, complete history or writer participation failed
- Failure: process exit 1; canonical state, complete history or writer participation failed
- Failure: process exit 1; canonical state, complete history or writer participation failed
- Failure: process exit 1; canonical state, complete history or writer participation failed
- Failure: process exit 1; canonical state, complete history or writer participation failed
- Failure: process exit 1; canonical state, complete history or writer participation failed
- Failure: process exit 1; canonical state, complete history or writer participation failed

Loss is seeded application-level outbound frame discard. The partition cuts communication between two halves during editing; both halves continue writing. Accepted packets still travel over real UDP. There is no emulated RTT, process crash or multi-host network in this suite.

CPU/RSS sidecars cover the entire child process. Canonical checks happen after writers stop and the settling deadline expires. A pass establishes this workload and deadline, not unbounded history growth, arbitrary WAN behavior or browser room capacity.
