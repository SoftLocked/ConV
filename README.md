# ConV

ConV is a local-first collaborative plain-text editor built with Rust,
WebAssembly, React, CodeMirror and WebRTC. Each browser owns a document replica,
applies edits locally, and exchanges changes with peers. It supports offline
editing, local persistence, remote cursors, text import/export and CRDT backups.

**Status:** research prototype intended for cloning and local use. Small-room
collaboration has automated coverage. The overlay admits up to 50 sessions, but
larger-room mixed-edit and stress scenarios still have completion-deadline
failures. See [current status](docs/status.md) and [limitations](docs/limitations.md).

## Clone and run

Use Linux or WSL with Bash, Rust/rustup, Node.js 24 and a C compiler. The repository
pins Rust 1.90.0 and pnpm 10.28.2. On Ubuntu/WSL, install build tools if needed:

```sh
sudo apt-get update
sudo apt-get install build-essential pkg-config
```

Then:

```sh
git clone https://github.com/SoftLocked/ConV.git
cd ConV
rustup toolchain install 1.90.0 --profile minimal --component rustfmt --component clippy
rustup target add wasm32-unknown-unknown --toolchain 1.90.0
cargo install wasm-bindgen-cli --version 0.2.129 --locked
npm install -g pnpm@10.28.2
pnpm install --frozen-lockfile
pnpm dev
```

Install Rust through [rustup](https://rustup.rs/) if it is not available, and open
a new shell afterward. The first build can take several minutes. Docker, a domain
and a hosting account are unnecessary for local use. No `.env` file is required.

Open **http://127.0.0.1:5173**. The development command builds WASM and starts the
web app and local signaling service; Ctrl+C stops both. Restart it after Rust
changes. TypeScript changes reload automatically. If the default ports are in
use, run a separate local instance with:

```sh
CONV_APP_PORT=5275 CONV_SIGNAL_BIND=127.0.0.1:3102 pnpm dev
```

Then open `http://127.0.0.1:5275`.

## Try collaboration

1. Open the app in a normal browser window.
2. Copy the complete URL, including its `#room` portion, into an incognito window
   or a different browser profile. Independent profiles have separate storage.
3. Edit different words or sentences in the same paragraph and watch changes
   appear in both windows.
4. In one window, select **Connection details → Work offline**. Edit on both
   sides, then select **Reconnect** and check that both versions agree.

Signaling introduces peers; document changes travel over WebRTC. Each profile
saves its own history in IndexedDB. Wait for **Saved on this device** before
closing, and export a **Backup** before clearing browser storage. Text import
creates a new room; a CRDT backup preserves identity and history.

A room link is a bearer invitation: anyone with it can read and edit. A newcomer
needs a reachable replica holding the document; there is no cloud document store.
Generic undo is disabled until collaboration-aware undo is implemented. Sharing a
localhost URL with a different physical computer will not connect it to your app.

## Run checks

From the repository root, after dependency installation:

```sh
cargo fmt --all -- --check
cargo test --locked --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo build --locked -p conv-native-peer -p conv-signaling
python3 tests/native/smoke.py
./scripts/build-wasm.sh
pnpm build
node --test tests/network/topology.test.mjs
pnpm --dir web exec playwright install --with-deps chromium
pnpm test:browser
```

Stop `pnpm dev` before browser tests: Playwright starts its own servers on ports
3001, 5173 and 5174. To leave a development server running, select unused test
ports instead:

```sh
CONV_TEST_APP_PORT=5273 CONV_TEST_SIGNAL_PORT=3101 CONV_TEST_PREVIEW_PORT=5274 pnpm test:browser
```

Browser installation may prompt for sudo to install Linux libraries. TURN verification is optional locally and is skipped unless
`CONV_TURN_URL` and `CONV_TURN_SECRET` point to a configured relay. CI provisions
a disposable loopback relay for that test.

Tests cover CRDT/reference agreement, merge algebra, causal holes, partitions,
SWIM, chunking and buffer pressure, native restart, Unicode/composition, offline
editing, failed saves, backups and multi-hop browser collaboration. Expensive
50-browser and performance tests require explicit flags; they are not part of
the default browser run.

## Experiments and design

The engine also runs native UDP experiments. Earlier 200-peer measurements used
ten active writers. The all-200-writer stress suite initially failed its settling
deadline; failed runs remain in the repository. Native task/socket results and
single-host browser checks are separate from WAN or production capacity claims.

- [Architecture](docs/architecture.md), [CRDT specification](docs/crdt-spec.md)
  and [wire protocol](docs/wire-protocol.md).
- [Browser adapter](docs/browser-architecture.md), [overlay](docs/browser-overlay.md)
  and [membership](docs/membership.md).
- [Measured results](benchmarks/reports/current/report.md),
  [measurement methods](docs/benchmark-methodology.md) and
  [stress testing](docs/stress-testing.md).
- [Optional demo scripts](docs/demo.md) and
  [optional hosting configuration](docs/deployment.md).

The project is released under the [MIT license](LICENSE). Bundled font licenses
are in [web/public/licenses](web/public/licenses).
