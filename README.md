# ConV

A proposed local-first, peer-to-peer collaborative plain-text editor with a
custom Rust replication engine shared by native peers and browser WebAssembly.

Implementation is at the early replication milestone. The repository contains
a draft CRDT specification, a reference document model, in-memory mutation
history, periodic range repair, and a deterministic delivery harness. There is no
browser editor, transport encoding, network connection, persistence, or benchmark
result yet.

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
in-memory messages. The original `partition` example remains a full-state baseline.
Virtual delivery times are scenario inputs, not performance results. The
reference renderer prioritizes clarity and has quadratic traversal.

Next: version and encode network messages with validation and size limits, then
add snapshot bootstrap and SWIM membership. Persistence and the browser editor
follow the core protocol checks.

Node 24 is already installed. pnpm, the WASM target, and browser dependencies will
be configured during the browser milestone. Docker is unnecessary for core work;
for the later signaling/TURN stack, enable this distro under Docker Desktop's
Settings > Resources > WSL Integration.

See [the CRDT draft](docs/crdt-spec.md) and
[the full proposal](.claude/decentralized_text_editor_proposal.md).
