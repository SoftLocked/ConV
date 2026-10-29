# Decentralized Gossiping Text Editor

## Project proposal

**Working name:** ConV<br>
**Owner:** Harikrishna Vardhineedi<br>
**Status:** Proposed rebuild from scratch<br>
**Primary stack:** Rust, WebAssembly, TypeScript, React, CodeMirror 6, WebRTC, SWIM, custom CRDT<br>
**Deliverable:** A hosted collaborative plain-text editor, a reusable Rust replication engine, and reproducible correctness and performance experiments.

## 1. Project summary

Build a collaborative text editor where each participant owns a local document replica, edits without waiting for a server, and exchanges changes directly with other participants. Users can create a document, share a room link, collaborate live, disconnect, continue writing, and reconcile their edits when they reconnect.

The engineering centerpiece is a Rust engine shared between a browser WASM build and a native test harness. It implements a sequence CRDT, delta-state synchronization, anti-entropy repair, and SWIM membership. The web application makes the system usable, while the native harness makes failures, scaling behavior, and performance measurable.

The first usable release supports **2-8 browser peers per room**. A subsequent systems milestone validates **200+ native peers** running the same replication engine. Supporting 200 simultaneous browser peers is a separate stretch milestone that requires a bounded connection overlay and browser-specific testing.

This proposal uses the resume's existing numbers as benchmark targets to investigate. None of those numbers is an established result of the rebuild.

## 2. Product and engineering goals

### Product goals

- Let someone start writing and invite a collaborator in under a minute, without creating an account.
- Keep typing responsive when the network is slow or unavailable.
- Preserve local work across refreshes and browser restarts, subject to browser storage availability.
- Recover from temporary partitions without asking users to resolve text-file conflicts manually.
- Provide a useful tool for shared notes, interview practice, small coding sessions, and collaborative Markdown drafts.

### Engineering goals

- Implement the replication and membership algorithms ourselves in Rust.
- Reuse the same core in WASM, deterministic simulations, and native network experiments.
- Demonstrate convergence despite duplicate, reordered, delayed, and temporarily lost messages.
- Measure bandwidth savings against a precisely defined full-state baseline.
- Make partition recovery, membership changes, and synchronization behavior inspectable.
- Produce a repository that supports a detailed distributed systems interview discussion.

### Scope boundaries

The initial release excludes rich text, comments, accounts, cloud document storage, permissions, code execution, AI features, Git integration, attachments, collaborative undo, and production-grade malicious-peer defenses. A collaboration-aware undo design can follow the first release; generic editor undo must not silently overwrite collaborators' changes.

The editor accepts plain text, including Markdown source. It does not need a Markdown preview to launch.

## 3. What decentralization means here

**Document authority is distributed.** There is no central document server that stores the canonical text, orders edits, or decides merge results. Each replica applies local edits and merges remote state independently.

The hosted browser version still needs supporting infrastructure:

| Component | Responsibility | Holds document content? |
| --- | --- | --- |
| Static web host | Deliver the application and WASM assets | No live document state |
| Signaling service | Exchange room membership hints, SDP offers/answers, and ICE candidates | No document synchronization messages |
| STUN service | Help discover network connectivity | No |
| TURN service | Relay encrypted WebRTC traffic when direct connectivity fails | Encrypted traffic passes through it |
| Participant devices | Edit, store, merge, and forward document state | Yes |

Describe the result as **peer-to-peer collaboration without a central document server**. Do not claim that an Internet-facing browser deployment needs no servers.

Established connections should continue working during a signaling outage while those connections remain healthy. New joins, connection repair, and some network changes can still require signaling. Turning off the signaling process does not demonstrate independence from an active TURN relay.

Document availability also depends on replicas: a brand-new participant cannot recover a document if no reachable participant has a copy. A room link is an invitation, not a cloud backup.

## 4. User experience

### Primary flow

1. Open the app and create a document.
2. Begin editing immediately; persist the document locally.
3. Copy an unguessable room link and share it.
4. Another browser opens the link and connects to an existing peer.
5. The newcomer receives CRDT state and joins live editing.
6. Participants see remote cursors, names, and connection status.
7. A disconnected participant keeps editing locally.
8. When connectivity returns, anti-entropy exchanges missing state and all replicas converge.

### Interface

- A large CodeMirror editor with basic syntax highlighting and keyboard navigation.
- A compact toolbar: document name, invite link, import, and export.
- A participant list with colors and connection status.
- Clear statuses: `Saved on this device`, `Saving locally`, `Offline`, `Connecting`, and `Synchronizing`.
- An optional diagnostics panel showing direct peer connections, local membership observations, pending sync bytes, and recent protocol events.

Local persistence and replication are different statuses. A network acknowledgement does not imply a durable remote backup. Do not display a global "everyone is synchronized" claim from a single neighbor's acknowledgement.

Remote cursor positions are ephemeral. They should expire when a peer becomes inactive and should not be persisted in document history.

## 5. Architecture and technology choices

| Layer | Proposed implementation | Reason |
| --- | --- | --- |
| Editor UI | React, TypeScript, Vite, CodeMirror 6 | Familiar web stack with a mature text editing surface |
| Browser adapter | TypeScript Web Worker, WebRTC, IndexedDB | Isolate protocol work and use browser-native networking/storage |
| WASM boundary | Rust and wasm-bindgen | Expose the same core used by native experiments |
| Document model | Custom Rust sequence CRDT | Make replication behavior a substantive implementation contribution |
| Sync protocol | Binary delta fragments, causal summaries, anti-entropy | Avoid repeated full-state transfers and recover missed changes |
| Membership | Rust SWIM state machine | Implement randomized probing, indirect probes, and suspicion |
| Signaling | Small Rust service using Tokio and Axum | Keep infrastructure small and use one systems language |
| Native peer adapter | Tokio UDP sockets and file-backed local persistence | Run real networked peers without launching hundreds of browsers |
| Verification | Rust property tests, deterministic simulator, Playwright | Cover algorithm correctness and actual browser behavior |
| Packaging | Cargo workspace, pnpm workspace, Docker Compose, CI | Reproducible development and deployment |

Use CodeMirror for rendering and input handling. Do not implement a text rendering engine. Implement the document CRDT and synchronization ourselves rather than delegating those responsibilities to Yjs, Automerge, or a hosted collaboration product.

Keep the Rust core independent of Tokio, browser APIs, wall-clock time, and disk. The host provides time, randomness, transport events, and persistence results. The core produces effects such as `Send`, `ScheduleTimer`, `Persist`, and `ApplyEditorChanges`.

Use lockfiles and pin the Rust toolchain during implementation. Select compatible dependency versions when the repository is initialized; this proposal does not prescribe unverified version numbers.

## 6. Document CRDT

### Recommended initial design

Implement a tombstone-based sequence CRDT with RGA-style insertion anchors. Represent its state as immutable insertion records and a grow-only deletion set. Merge joins these records and deletion markers; received fragments must be valid partial states rather than position-based edit commands.

Each insertion record identifies its character, stable element ID, predecessor anchor, and deterministic ordering metadata. Deletes refer to stable element IDs. Deleting a character hides it without destroying the anchor used by concurrent or later insertions.

Before implementation, write a short algorithm specification defining the state, merge, traversal, insertion ordering, and dependency handling. Include concrete examples of inserting in the middle of existing text and concurrent insertion at the same position. An arbitrary sort by peer ID is insufficient to preserve expected local insertion behavior.

Use separate concepts for:

- **Replica identity:** a random ID for a document's active writer session.
- **Mutation identity:** a contiguous per-replica sequence number used for deduplication and synchronization.
- **Element identity:** a mutation ID plus an offset when a transaction inserts several characters.
- **Ordering metadata:** logical time and deterministic tie breakers, with a documented comparison rule.

Do not use a Lamport clock's highest observed value as evidence that every earlier mutation has arrived.

### Required semantics

- Local inserts and deletes become visible without a network round trip.
- Repeated delivery is harmless.
- Merge is associative, commutative, and idempotent over valid states.
- A delete received before its insertion remains effective when the insertion arrives.
- A child received before its predecessor is retained until its dependency arrives.
- Deleting an anchor does not delete its descendants.
- Concurrent edits are retained according to documented deterministic sequence rules.
- Rendering the same complete state always produces the same text.

Eventual convergence is conditional on surviving replicas eventually exchanging all required state after connectivity returns and edits stop. It does not guarantee that every concurrent edit matches every author's intent.

### Unicode and editor integration

Start with one Unicode scalar value per logical element. CodeMirror uses JavaScript string positions, so explicitly map between UTF-16 offsets and CRDT elements. Test emoji, surrogate pairs, combining marks, multiline replacements, paste, and IME composition. Grapheme-aware cursor behavior belongs in the UI adapter.

Translate local editor transactions into CRDT mutations. Tag remote editor transactions so they do not generate duplicate local mutations. Apply incremental changes and preserve selection anchors instead of replacing the entire editor document after every message.

Remote cursor anchors should refer to stable elements plus a left/right affinity. Resolve deleted anchors deterministically.

### Storage and history

Persist CRDT state, causal summaries, and pending local mutations in IndexedDB using atomic transactions. Show `Saved on this device` only after the relevant transaction succeeds. Surface quota and write failures. Work that has not completed local persistence can be lost if the tab crashes.

Create a fresh writer identity after a new session restores a saved document. Multiple tabs must not allocate IDs from the same writer counter. Existing element IDs remain unchanged across restoration.

Keep tombstones and all state necessary for old replicas to rejoin. Snapshots can speed loading, but do not remove causal history merely because currently connected peers have acknowledged it. Safe garbage collection with indefinitely offline replicas is a later protocol problem.

## 7. Delta synchronization and anti-entropy

### Fast path

1. Convert an editor transaction into a local delta-state fragment.
2. Apply it locally and request persistence.
3. Batch outgoing fragments briefly, with an initial target of 10-20 ms.
4. Send to a small random fanout of peers.
5. Merge and deduplicate received fragments, forwarding new information within a bounded dissemination budget.
6. Use periodic anti-entropy to repair anything missed by the fast path.

Piggyback small fragments on SWIM probes and acknowledgements when space permits. Also send explicit delta messages so typing does not wait for the next probe period. Membership and failure-detection messages receive priority over bulk synchronization.

### Repair path

Peers periodically exchange compact causal summaries and request missing mutation ranges. Track contiguous prefixes and out-of-order ranges per replica. A summary must never acknowledge holes as received.

Keep indexed delta history for range repair. A new peer or a peer far behind can request a complete CRDT snapshot, including tombstones and ordering metadata. Serialize snapshots into bounded chunks with IDs, integrity checks, timeouts, and bounded reassembly memory. Request missing chunks or restart an incomplete transfer; receiving an initial chunk must not mark the entire snapshot as integrated.

Preserve local edits made while a snapshot is arriving. Installing a snapshot means merging it, not replacing a more recent local replica.

### Suggested message families

| Message | Purpose |
| --- | --- |
| `Hello` | Validate protocol version, room, and session identity |
| `Ping`, `Ack`, `PingReq` | Direct and indirect membership probes |
| `MembershipUpdate` | Disseminate join, suspicion, failure, and leave observations |
| `Delta` | Transfer a bounded partial CRDT state |
| `SyncSummary`, `SyncRequest` | Identify and request missing mutation ranges |
| `SnapshotOffer`, `SnapshotChunk`, `ChunkRequest` | Bootstrap and repair large gaps |
| `Presence` | Transfer temporary cursor and participant information |

Every envelope includes a protocol version, room ID, sender session, and message ID. Keep membership incarnation numbers separate from document mutation counters. Use a single documented binary encoding, initially Serde with Postcard, and validate all lengths and counts before allocation.

Start with UDP frames below 1,200 bytes including the application envelope. Chunk larger data at the application layer. Browser frames may use a larger configured cap, but never assume WebRTC accepts arbitrarily large messages.

Use bounded queues, fair scheduling, and transport backpressure. Presence can be dropped or replaced by newer presence. Document changes may leave the immediate gossip queue only if they remain recoverable from retained state and anti-entropy. Never discard document history to satisfy a queue limit.

## 8. SWIM membership and transport

### Protocol behavior

Implement SWIM's separate failure-detection and membership-dissemination responsibilities:

1. Probe peers in a shuffled traversal rather than repeatedly choosing an unrestricted random target.
2. If a direct probe times out, ask a small helper set to probe the target.
3. Mark the peer `suspect` if no acknowledgement arrives.
4. Permit the peer to refute suspicion with a higher incarnation number.
5. Mark it `dead` after the suspicion timeout and disseminate the observation.

Specify precedence for membership updates and reject older incarnation information. A restarted peer uses a fresh session identity. Voluntary leave is distinct from an unresponsive peer.

Failure detection is an observation, not proof of a crash. A partition or throttled background browser tab can look like a failed process. Suspecting or removing a connection must never delete its document edits or prevent a later merge.

Initial tunable defaults: one-second probe period, 250 ms direct timeout, up to three helpers, and a five-second suspicion period. These are starting points for foreground experiments, not WAN guarantees. Re-evaluate them under browser timer throttling and measured RTT.

### Browser transport

Use WebRTC data channels. Start with a full mesh capped at eight peers, where each member can directly probe every other member. Reliable ordered channels carry document synchronization; separate short-lived/unordered traffic can carry presence and probe messages. Prioritize control traffic, while recognizing that channels can still share an underlying connection and congestion budget.

Use deterministic offer initiation or the WebRTC perfect-negotiation pattern to handle simultaneous joins. Queue early ICE candidates, handle closed channels and ICE restarts, and provide a visible connection failure state. Test both direct connectivity and forced TURN relay.

### Native transport and scaling

Run the same core using UDP-addressable native peers. Supply seed endpoints through local configuration. Retain anti-entropy and chunk repair because UDP delivery is not reliable.

The native 200-peer milestone does not require 199 persistent browser connections per user. It exercises the replication engine and membership behavior under a transport where arbitrary known peers can be addressed directly.

**Future browser scale:** introduce a bounded connection overlay, peer sampling, reconnection, and a defined way to route probes or change the membership model. Native SWIM scalability does not establish browser-overlay scalability. Do not present a neighbor-only failure detector as unchanged full-group SWIM.

## 9. Signaling, security, and deployment

Keep signaling ephemeral: track room sessions and relay only connection metadata. Expire stale entries, bound room size, validate origins and messages, and rate-limit joins and offers. Never send document payloads over signaling as an implicit fallback.

Deploy the UI over HTTPS, signaling over WSS, and a configured TURN service with expiring credentials. Do not ship permanent TURN secrets in the client. Use Docker Compose for the signaling/TURN development stack and environment-based configuration for deployment.

Treat room links as bearer invitations: anyone who obtains one may read and edit the room. The first release assumes cooperative participants. WebRTC protects individual transport links, but a signaling service that is malicious or compromised is outside the initial authentication guarantees. Do not market the application as a zero-knowledge or independently authenticated end-to-end encrypted collaboration system.

Render imported text and participant names as untrusted text. Limit message size, snapshot size, pending dependencies, and per-peer resource use. Do not log document contents, invitation tokens, full SDP, or ICE candidates in production diagnostics.

An offline-capable application shell should be cached after the first visit. Browser eviction, cleared storage, and unavailable replicas remain explicit availability limits. Offer plain-text export and a versioned CRDT backup format that preserves document identity and history. Importing plain text creates a new document; it does not restore an old room's synchronization identity.

## 10. Validation strategy

### CRDT and persistence

- Unit tests for sequential editing, same-anchor inserts, overlapping deletes, deleted anchors, and delete-before-insert.
- Property tests for merge algebra and convergence under permutations, batching, and duplication.
- Assert canonical CRDT state equality as well as rendered text equality; invisible metadata divergence can break later edits.
- Replay every saved failing random seed as a regression fixture.
- Test crash boundaries around persistence, restoration, and identifier allocation.
- Differentially check the optimized implementation against a simple reference implementation written for clarity.

### Deterministic network simulator

Build a virtual clock and seeded scheduler that run the real core and encoded messages. Inject latency, duplication, loss, reordering, churn, partitions, and reconnects. Model protocol timers rather than advancing them with real sleeps.

Required scenarios include a three-way partition with edits on every side, a peer rejoining after 5,000 mutations, a missing predecessor, a stale suspicion message, and a new peer bootstrapping during active editing. After connectivity is restored and editing stops, drain or repair state and require every surviving replica to converge.

### Real browser tests

Use Playwright browser contexts to test joins, typing, reload, persistence, cursor mapping, and offline/reconnect behavior. Validate Unicode and composition input manually where automation cannot reproduce OS-level behavior faithfully.

Also test separate devices across two actual networks, forced TURN, background tabs, and established editing during a signaling outage. Simulator partitions and browser DevTools offline mode alone do not prove real WebRTC behavior.

### Native network tests

Run 10, 50, 100, and 200 native peers over real sockets. Record whether peers are separate processes or tasks inside one process. On a single PC, label results as loopback experiments and record CPU/memory contention. Add multi-host tests only when equipment is available.

## 11. Benchmarks and success criteria

Correctness is a release gate. Performance numbers are optimization goals and must be reported with the workload that produced them.

| Resume-aligned target | Measurement contract |
| --- | --- |
| 200+ peers | First demonstrate 200 native replicas converging under a declared edit load. Report browser room capacity independently. |
| Sub-1 ms merge | Target p99 incremental CRDT merge below 1 ms for a small live-edit delta on a 100 KiB document. Measure native and WASM separately; exclude rendering and network time, then report their costs separately. |
| 95% payload reduction | Compare delta-state replication with sending the complete serialized CRDT state at the same batching cadence, topology, and reliability settings. Report all sync bytes and a separate transport-overhead view. |
| About 15 KB/s per node | Evaluate a declared workload, initially 200 native peers, ten active writers, and five single-character mutations per writer per second. Count both sent and received application bytes, including forwarding and repair; report mean and p95 per-node rates. |
| Under 200 ms p99 recovery | Stretch target for eight native peers on a declared low-latency loopback/LAN setup after one peer accumulates 5,000 offline mutations. Measure from restored bidirectional reachability to all target peers having identical state, with writers stopped. Report WAN and browser results separately. |

Additional product targets: local key-to-paint p95 below 50 ms on a documented device and remote edit visibility p95 below 200 ms in an eight-peer foreground room with a declared network profile.

Use at least 1,000 measured reconnect trials before drawing a p99 recovery conclusion. Use warm-up runs and multiple seeds. Describe a "mutation" precisely; distinguish 5,000 user transactions from 5,000 inserted characters.

Run scaling experiments at 1 KiB, 100 KiB, and 1 MiB of visible text, reporting serialized history size and tombstone count. Include steady editing, large paste, deletion-heavy history, idle rooms, reconnect bursts, and simultaneous writers. Single-character CRDT metadata may become expensive on large documents; measure before introducing block compression.

Each report records commit, toolchain, hardware, browser, transport, peer count, active writers, edit rate, batch interval, network profile, duration, seed, bytes sent/received, CPU, memory, and p50/p95/p99 latency. Use one clock in the local harness; do not subtract unsynchronized browser wall clocks to claim network latency.

Bandwidth reduction is `1 - delta_bytes / full_state_bytes`. Include initial bootstrap in whole-session comparisons and report steady-state comparisons separately. Full-state replication is a deliberately simple baseline, not evidence of superiority over mature CRDT libraries. Do not claim "minimal network cost" without a defined comparison.

## 12. Implementation milestones

The sequence below is sized for approximately six weeks of focused solo work. Exit criteria control progression; the calendar is an estimate.

| Milestone | Scope | Exit criteria |
| --- | --- | --- |
| 1. Specify and implement the core | Algorithm specification, simple reference model, sequence state, merge, serialization | Sequential semantics and property tests pass; failing seeds replay |
| 2. Prove replication behavior | Delta extraction, causal summaries, anti-entropy, virtual network, SWIM | Partition/rejoin scenarios converge; stale membership updates are handled |
| 3. Build a local browser editor | WASM worker, CodeMirror adapter, IndexedDB, import/export | Unicode editing and refresh recovery work without networking |
| 4. Ship browser collaboration | Signaling, WebRTC, cursors, snapshots, connection states | 2-8 peers collaborate; direct and TURN paths pass; established connections survive signaling loss |
| 5. Validate scale and optimize | Native UDP peers, 200-peer runs, bandwidth baseline, profiling | Reproducible result files and correctness checks for every benchmark |
| 6. Package and publish | Deployment, documentation, demo, CI, failure scenarios | Hosted demo and a fresh-clone reproduction path are verified |

Build a two-replica vertical slice early, but do not optimize away metadata or implement a browser overlay before the reference model and fault-injection tests are reliable.

If time is constrained, deliver the working eight-peer product and its correctness evidence first. Keep uncompleted 200-peer and latency goals explicitly marked as pending.

## 13. Repository structure

```text
ConV/
  Cargo.toml
  rust-toolchain.toml
  pnpm-workspace.yaml
  crates/
    document/             # Sequence state, merge, traversal, encoding
    protocol/             # Delta sync, membership, timers, effects
    wasm/                 # Thin bindings over the shared engine
    simulator/            # Virtual network and reproducible failures
    native-peer/          # UDP transport, persistence, workload driver
    signaling/            # Ephemeral room discovery and SDP/ICE relay
  web/
    src/editor/           # CodeMirror transaction and position adapters
    src/worker/           # WASM host and core event loop
    src/network/          # WebRTC and signaling adapters
    src/storage/          # IndexedDB persistence and restore
    src/components/
  tests/
    browser/
    fixtures/
  benchmarks/
    workloads/
    raw-results/
    reports/
  infra/
    compose.yaml
    turn/
  docs/
    architecture.md
    crdt-spec.md
    wire-protocol.md
    benchmark-methodology.md
    limitations.md
  README.md
```

Keep modules cohesive rather than creating a crate for every small abstraction. Document one-command development, correctness checks, browser checks, and benchmark execution after those commands exist.

## 14. Risks and planned responses

| Risk | Response |
| --- | --- |
| Convergent text with poor insertion behavior | Specify sequential semantics and concurrent examples before coding; compare against the reference model |
| Tombstone and history growth | Report memory and history size; retain correctness metadata; defer safe garbage collection |
| 200-peer browser connection cost | Cap the initial browser room; validate native scale separately; treat an overlay as a new milestone |
| Lost delta fragments or incomplete snapshots | Periodic anti-entropy, exact missing ranges, bounded chunk retries |
| Background tab suspension | Treat membership as provisional; resynchronize on resume; test timer throttling |
| Slow peer causes unbounded queues | Bounded buffers, fair scheduling, coalesced presence, repair from retained state |
| Local storage fails or is evicted | Clear persistence status, export/backup, no promise of cloud durability |
| Native metrics do not predict browser behavior | Separate reports for simulator, native sockets, WASM, and real browser sessions |

## 15. Deliverables and demonstration

The finished project includes source code, a hosted editor, local development setup, a CRDT specification, a wire protocol description, replayable failure scenarios, raw benchmark results, and a short demo video.

The demonstration should show:

1. Two devices joining through a shared link and editing simultaneously.
2. One device disconnecting while both sides keep writing.
3. Reconnection and deterministic convergence without dropping either side's retained edits.
4. Continued editing over established connections while signaling is unavailable.
5. A separate 200-native-peer run with visible convergence checks and exported measurements.

The README must distinguish supported product behavior, laboratory measurements, and stretch targets. Resume bullets should use the measured results and identify the tested transport and peer type when that distinction matters.

## 16. Implementation handoff

When this proposal is approved for coding, begin with milestones 1 and 2. Establish the Cargo workspace, write the CRDT specification and reference implementation, add merge/property tests, and build a two-peer deterministic sync test. Then add fault injection and membership before wiring up the browser application.

Preserve these constraints throughout implementation:

- Implement the Rust CRDT, delta replication, and membership core ourselves.
- Keep the core transport-independent and shared between native and WASM.
- Keep document contents out of signaling and centralized storage.
- Retain state needed for offline recovery; do not make unsafe compaction shortcuts.
- Do not replace peer-to-peer document exchange with a WebSocket document backend.
- Do not hardcode successful metrics, fake peers, or simulated results as live product behavior.
- Make each milestone runnable and verifiable before proceeding.

## References

These sources ground the protocol choices. The architecture, scope, tuning values, and performance targets above are proposed engineering decisions, not results established by these sources.

1. [Das, Gupta, and Motivala: SWIM](https://www.cs.cornell.edu/projects/Quicksilver/public_pdfs/SWIM.pdf). Primary reference for probing, suspicion, incarnation numbers, and membership dissemination.
2. [Almeida, Shoker, and Baquero: Delta State Replicated Data Types](https://arxiv.org/abs/1603.01529). Primary reference for joinable partial states and anti-entropy.
3. [RFC 8831: WebRTC Data Channels](https://www.rfc-editor.org/rfc/rfc8831.html). Transport architecture, reliability modes, and DTLS protection.
4. [WebRTC: Getting started with peer connections](https://webrtc.org/getting-started/peer-connections). Signaling, ICE, STUN, and TURN responsibilities.
5. [wasm-bindgen guide](https://rustwasm.github.io/docs/wasm-bindgen/). Rust/JavaScript interoperability.
6. [CodeMirror reference manual](https://codemirror.net/docs/ref/). Editor state, changes, transactions, and extension APIs.
