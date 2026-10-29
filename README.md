# ConV

A proposed local-first, peer-to-peer collaborative plain-text editor with a
custom Rust replication engine shared by native peers and browser WebAssembly.

Implementation has started at milestone 1. The repository currently contains a
sequence CRDT draft specification, a reference model with local editing, and a
deterministic document-delivery harness.
There is no browser editor, networking, persistence, or benchmark result yet.

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

The harness uses in-memory document fragments and explicit full-state repair.
Encoded messages, causal range repair, periodic timers, SWIM, and real sockets
are not implemented. Virtual delivery times are scenario inputs, not performance
results. The reference renderer prioritizes clarity and has quadratic traversal.

Next: validate insertion dependencies, introduce versioned mutation envelopes and
serialization, then implement causal summaries and range repair before SWIM.

Node 24 is already installed. pnpm, the WASM target, and browser dependencies will
be configured during the browser milestone. Docker is unnecessary for core work;
for the later signaling/TURN stack, enable this distro under Docker Desktop's
Settings > Resources > WSL Integration.

See [the CRDT draft](docs/crdt-spec.md) and
[the full proposal](.claude/decentralized_text_editor_proposal.md).
