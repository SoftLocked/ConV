#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
conv_app_port="${CONV_APP_PORT:-5173}"
conv_signal_bind="${CONV_SIGNAL_BIND:-127.0.0.1:3001}"
if [[ ! "$conv_app_port" =~ ^[1-9][0-9]{3,4}$ ]] || ((conv_app_port < 1024 || conv_app_port > 65535)); then
  echo 'CONV_APP_PORT must be a port between 1024 and 65535' >&2
  exit 1
fi
cargo build -p conv-signaling
./scripts/build-wasm.sh
CONV_ORIGIN="${CONV_ORIGIN:-http://127.0.0.1:$conv_app_port}" CONV_SIGNAL_BIND="$conv_signal_bind" ./target/debug/conv-signaling &
conv_signal_pid=$!
VITE_SIGNAL_URL="${VITE_SIGNAL_URL:-ws://$conv_signal_bind/signal}" VITE_ICE_URL="${VITE_ICE_URL:-http://$conv_signal_bind/ice}" setsid pnpm --dir web dev --port "$conv_app_port" --strictPort &
conv_web_pid=$!
cleanup() {
  kill "$conv_signal_pid" 2>/dev/null || true
  kill -- "-$conv_web_pid" 2>/dev/null || true
}
trap cleanup EXIT INT TERM
wait -n "$conv_signal_pid" "$conv_web_pid"
