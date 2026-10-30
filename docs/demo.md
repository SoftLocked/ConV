# Record the local demonstration

Build WASM, debug signaling and release experiment binaries, install pnpm/browser
dependencies, then run:

```sh
./scripts/build-wasm.sh
cargo build -p conv-signaling
cargo build --release -p conv-native-peer --bins
node scripts/record-demo.mjs
```

The script owns its local signaling/Vite processes (ports 3005/5175) and cleans
them up. It uses Playwright's installed Chromium and records one continuous
`demo/conv-demo.webm` with two clearly separated scenes:

1. Alice and Bob join independent contexts, exchange edits, retain edits on both
   sides of a disconnect, reconnect, then keep editing after the actual signaling
   process stops.
2. A separately labeled view of the actual 200-socket UDP experiment and its
   convergence/traffic report. It exports `demo/native.json`.

These scenes are local automated contexts and loopback native tasks. They do not
show physical devices on independent networks; complete the manual release
checks before describing them that way. The native scene does not display fake
browser participants or substitute simulated timing for network measurements.

The recording needs no external video editor or FFmpeg installation. Videos are
generated local artifacts and are ignored by Git. Re-record from a committed
source tree if the native report is intended as published evidence.

## Five browsers visible together

```sh
node scripts/record-five-browsers.mjs
```

This creates `demo/five-browsers.webm`, a final PNG and a JSON verification
report. Five independent Chromium contexts run the actual application, each
with isolated storage and its own WebRTC connections. A sixth context records
a dashboard of their live viewport screenshots. No document DOM is copied
between clients. Compact demo-only CSS makes every editor readable in the grid.

The scenario checks every expected contribution, simultaneous typing on five
separate checklist lines, and offline edits on both sides of Eve's disconnect.
Every checkpoint compares all five actual editor texts with the expected text.
This local visual demonstration is not a browser latency benchmark. Ports
3006/5176 must be free; owned services are stopped on exit.

## Organic shared-paragraph scenario and 50 real sessions

```sh
node scripts/record-five-browsers.mjs --organic --participants 5
node scripts/record-five-browsers.mjs --organic --participants 50
```

The five visible writers edit different selections within one shared short
paragraph. They insert, delete, rewrite and correct typos left by each other.
The 50-session version creates 45 additional isolated Chromium contexts running
the full app. They add and rewrite review notes below the visible paragraph,
with a declared 120 ms spacing between background edits. Every client’s complete
text and exported CRDT history are checked. This is a functional demo, distinct
from simultaneous burst-load stress. It writes `demo/organic-50-browsers.webm`,
a final PNG and a JSON proof; use the corresponding `organic-5-browsers` files
for the smaller version. Failures write diagnostic JSON instead of a success
report. Fifty contexts require substantial RAM and can use swap on this host.
