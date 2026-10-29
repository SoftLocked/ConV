# ConV

ConV is a local-first collaborative plain-text editor. Each participant owns a
Rust CRDT replica, edits immediately, and exchanges changes over WebRTC. It
supports rooms of **up to eight browser participants**, offline edits, local
recovery, remote cursors, plain-text import/export and history-preserving backups.
There is no central document store. Signaling and STUN/TURN support connectivity;
a room link cannot recover a document when no available replica retains a copy.

The shared Rust engine also runs deterministic failure simulations and native
UDP experiments with 200 replicas. Those are laboratory measurements, separate
from browser capacity. The app is verified locally; public deployment, Docker
execution here and physical-device/WAN checks remain pending. See
[delivery status](docs/status.md) and [limitations](docs/limitations.md).

## Run locally

Use Linux/WSL with Rust/rustup, a C linker/build tools, Node 24 and pnpm 10.28.2.
The repository pins Rust 1.90.0. On Ubuntu/WSL, install build tools with
`sudo apt-get install build-essential pkg-config`. After installing Rust, open a
new terminal or run `source "$HOME/.cargo/env"`.

From the repository root:

```sh
rustup target add wasm32-unknown-unknown
cargo install wasm-bindgen-cli --version 0.2.129 --locked
npm install -g pnpm@10.28.2
pnpm install --frozen-lockfile
pnpm dev
```

`pnpm dev` builds signaling and WASM, then runs both services. Open
**http://127.0.0.1:5173** and copy the invite link into another browser profile.
Ctrl+C stops the development services. Rust changes require restarting this
command; TypeScript changes reload automatically. Localhost is suitable for local
browser profiles; actual remote devices require an accessible HTTPS deployment.

Wait for **Saved on this device** before closing. Browser storage can be cleared
or evicted; export a **Backup** to preserve room identity, history and tombstones.
Plain-text import starts a new room. Anyone with a room link can read and edit;
the initial scope assumes cooperative participants. Generic undo is disabled
until collaboration-aware undo is designed.

## Check correctness

```sh
cargo fmt --all -- --check
cargo test --locked --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo build -p conv-native-peer -p conv-signaling
python3 tests/native/smoke.py
./scripts/build-wasm.sh
pnpm build
pnpm --dir web exec playwright install --with-deps chromium
pnpm test:browser
```

Tests cover CRDT merge algebra/reference agreement, reordered/missing
predecessors, exact causal holes, three-way partitions, 5,000-mutation reconnects,
SWIM suspicion/refutation, bounded decoding, snapshot/local-edit races, two
native processes with durable restart, Unicode, eight browser contexts,
simultaneous/offline edits, IME events, backups and cached offline reload.
Browser contexts simulate independent clients; physical-device checks are listed
[separately](docs/manual-validation.md). CI runs the same checks on GitHub; the
workflow has not been executed remotely in this session.

Forced TURN testing requires a running relay with expiring shared-secret
credentials:

```sh
CONV_TURN_URL=turn:YOUR_TURN_HOST:3478 CONV_TURN_SECRET=YOUR_SERVER_SECRET pnpm test:browser
```

The relay test checks the selected ICE candidate. It is skipped when TURN is
unconfigured. Keep the shared secret server-side; it is never a frontend build
variable. The local verification used a disposable loopback Coturn server.

## Native peers and failure demonstrations

In separate terminals, use separate state files:

```sh
cargo run -p conv-native-peer --bin conv-native-peer -- --bind 127.0.0.1:7001 --seed 127.0.0.1:7002 --state alice.conv
cargo run -p conv-native-peer --bin conv-native-peer -- --bind 127.0.0.1:7002 --seed 127.0.0.1:7001 --state bob.conv
```

Type lines to append, `/show` to inspect, and `/quit` to save and leave. Restart
with the same state file to restore under a fresh writer identity. A stale
`.conv.lock` may be removed only after its owning process has stopped. Native UDP
is an unencrypted cooperative test transport.

```sh
cargo run -p conv-simulator --example partition
cargo run -p conv-simulator --example automatic_repair
```

These examples use virtual clocks; their scenario delays are not performance
results.

## Reproduce measurements and demo

```sh
cargo build --release -p conv-native-peer --bins
python3 scripts/run-experiments.py
# Use the timestamped directory printed above:
python3 scripts/report-experiments.py --directory benchmarks/raw-results/TIMESTAMP
CONV_BENCH_BROWSER=1 pnpm --dir web test:browser --grep 'measure isolated'
node scripts/record-demo.mjs
```

Report rendering needs Python and Matplotlib (`python3 -m pip install matplotlib`
in a virtual environment). Run benchmarks serially on an otherwise idle host.
The runner retains raw reports and whole-process CPU/RSS sidecars; the full suite
includes 1,000 recovery trials. `--quick` is a smoke check and cannot qualify a
recovery p99. Run browser measurements before regenerating the report if you
want the latest browser result included.

[Measured results](benchmarks/reports/current/report.md) include native and WASM
merge latency, traffic at 10–200 native peers, matched full-state comparisons and
recovery. [Methods](docs/benchmark-methodology.md) define the measurement windows,
baselines, clock limits and reproducibility constraints. Targets remain separate
from observations; the approximate 15 KB/s active traffic target was exceeded.

The recording script creates `demo/browser-collaboration.webm` and
`demo/native-200.webm`. It records real browser collaboration/offline recovery/
signaling outage, followed by a separately labeled real UDP experiment. Browser
contexts in this recording are not physical devices. Generated videos stay local.

## Host the application

[Deployment instructions](docs/deployment.md) provide Caddy HTTPS/WSS, ephemeral
Rust signaling and Coturn with expiring credentials. Enable Docker Desktop's
WSL integration before trying the Compose stack here. A public host, DNS names
and TURN public IP are required for publication. Container execution and public
hosting have not yet been verified in this environment.

For implementation details, read the [architecture](docs/architecture.md),
[CRDT specification](docs/crdt-spec.md), [wire protocol](docs/wire-protocol.md),
[browser adapter](docs/browser-architecture.md), [membership](docs/membership.md)
and [original proposal](.claude/decentralized_text_editor_proposal.md).
