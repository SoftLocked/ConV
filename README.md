# ConV

A proposed local-first, peer-to-peer collaborative plain-text editor with a
custom Rust replication engine shared by native peers and browser WebAssembly.

Implementation is at the early replication milestone. The repository contains
a draft CRDT specification, a reference document model, in-memory mutation
history, periodic range repair, and a deterministic delivery harness. There is no
browser editor or benchmark result yet. A native UDP adapter supports durable
local history and restart recovery.

## Environment

Run commands inside this Linux/WSL environment. Install Rust using the official
instructions at https://rust-lang.org/tools/install/, then open a new shell or
run `source "$HOME/.cargo/env"`. The repository pins Rust 1.90.0; rustup will
install that toolchain when a Cargo command is first run. A C linker is required
(`cc` is already available in the inspected environment).

From the repository root:

```sh
cargo test --workspace
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
```

Validation: workspace tests, formatting, and Clippy pass on Rust 1.90.0.
The tests include merge algebra over exhaustive small-state combinations,
invalid predecessor rejection,
reordered/duplicated fragments, conflicting IDs, two- and three-peer partitions,
100 seeded delivery schedules, bootstrap during editing, and a 5,000-mutation
reconnect. These are correctness checks, not measured network benchmarks.

Run the deterministic partition demonstration:

```sh
cargo run -p conv-simulator --example partition
```

Run automatic repair after a simulated disconnection:

```sh
cargo run -p conv-simulator --example automatic_repair
```

The protocol peer emits sends and timer requests; the host provides the network
and clock. Periodic summaries trigger missing-range requests and bounded replies.
The simulator runs the same peer code using queued messages and a virtual clock,
including seeded loss, duplication, reordering, and partitions. This path uses
Postcard messages split into checksummed frames of at most 1,200 bytes. The original `partition` example remains a full-state baseline.
Virtual delivery times are scenario inputs, not performance results. The
renderer indexes insertion anchors for traversal.

Next: snapshot bootstrap, SWIM membership, and the local browser editor.

Node 24 is already installed. pnpm, the WASM target, and browser dependencies will
be configured during the browser milestone. Docker is unnecessary for core work;
for the later signaling/TURN stack, enable this distro under Docker Desktop's
Settings > Resources > WSL Integration.

See [the CRDT draft](docs/crdt-spec.md) and
[the full proposal](.claude/decentralized_text_editor_proposal.md).

## Native UDP peers

Use two terminals with separate state files:

```sh
cargo run -p conv-native-peer -- --bind 127.0.0.1:7001 --seed 127.0.0.1:7002 --state alice.conv
cargo run -p conv-native-peer -- --bind 127.0.0.1:7002 --seed 127.0.0.1:7001 --state bob.conv
```

Type a line to append it; `/show` prints the document and `/quit` saves and exits.
Restart with the same state file to restore history under a fresh writer ID.
Use distinct `--room` numbers to isolate rooms. UDP is a cooperative laboratory
transport, without encryption or authenticated membership. Browser transport
will use WebRTC. A process crash can leave a `.conv.lock` file; remove it only
after confirming the owning process has stopped.

```sh
cargo build -p conv-native-peer
python3 tests/native/smoke.py
```

The smoke test uses two real processes on loopback, including durable restart
and missing-edit recovery. It is not a scalability or WAN performance result.
