# Reproduction commands

```sh
cargo build --release -p conv-native-peer --bin conv-bench
# Run separately at 10, 50, 100 and 200 peers.
cargo run --release -p conv-native-peer --bin conv-bench -- --peers 200 --writers 10 --seconds 5 --settle-seconds 10 --document-bytes 1024 --output benchmarks/raw-results/native-200.json
# Document-size experiments on a memory budget: 10 peers at 100 KiB, 2 at 1 MiB.
cargo run --release -p conv-native-peer --bin conv-bench -- --peers 10 --writers 2 --document-bytes 102400 --output benchmarks/raw-results/native-100k.json
cargo run --release -p conv-native-peer --bin conv-bench -- --peers 2 --writers 1 --document-bytes 1048576 --output benchmarks/raw-results/native-1m.json
# Baseline pair: repeat the exact same arguments, then append --full-state.
```

Read [the methodology](../../docs/benchmark-methodology.md) before interpreting
these results. Do not substitute simulator time for measured network latency.
