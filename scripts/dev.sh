#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
cargo build -p conv-signaling
./scripts/build-wasm.sh
CONV_ORIGIN="${CONV_ORIGIN:-http://127.0.0.1:5173}" ./target/debug/conv-signaling &
conv_signal_pid=$!
setsid pnpm --dir web dev &
conv_web_pid=$!
cleanup() {
  kill "$conv_signal_pid" 2>/dev/null || true
  kill -- "-$conv_web_pid" 2>/dev/null || true
}
trap cleanup EXIT INT TERM
wait -n "$conv_signal_pid" "$conv_web_pid"
