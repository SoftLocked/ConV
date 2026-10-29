#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
cargo build -p conv-signaling
if [[ ! -f web/src/generated/conv_wasm_bg.wasm ]]; then ./scripts/build-wasm.sh; fi
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
