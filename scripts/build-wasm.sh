#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
cargo build -p conv-wasm --target wasm32-unknown-unknown --release
wasm-bindgen --target web --out-dir web/src/generated target/wasm32-unknown-unknown/release/conv_wasm.wasm
