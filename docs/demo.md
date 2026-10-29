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
them up. It uses Playwright's installed Chromium and records two scenes:

- `demo/browser-collaboration.webm`: Alice and Bob join independent contexts,
  exchange edits, retain edits on both sides of a disconnect, reconnect, then
  keep editing after the actual signaling process stops.
- `demo/native-200.webm`: a separately labeled view of the actual 200-socket UDP
  experiment and its convergence/traffic report. It exports `demo/native.json`.

These scenes are local automated contexts and loopback native tasks. They do not
show physical devices on independent networks; complete the manual release
checks before describing them that way. The native scene does not display fake
browser participants or substitute simulated timing for network measurements.

Both scenes have the same codec and dimensions. With FFmpeg installed, combine
them into a single video:

```sh
printf "file 'browser-collaboration.webm'\nfile 'native-200.webm'\n" > demo/scenes.txt
ffmpeg -f concat -safe 0 -i demo/scenes.txt -c copy demo/conv-demo.webm
```

Videos are generated local artifacts and are ignored by Git. Re-record from a
committed source tree if the native report is intended as published evidence.
