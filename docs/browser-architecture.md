# Browser adapter

A synchronous WASM replica translates CodeMirror UTF-16 transactions and renders
merged text. A second replica in a Web Worker owns protocol scheduling and local
persistence. Both merge identical immutable mutations. Only the synchronous
replica allocates document mutations; the worker never generates local edits.
Message IDs use separate low/high-bit lanes for worker and editor traffic.
Both copies share one session ID; refreshing creates a fresh random session.

Main-thread edits are applied synchronously and sent to the worker as mutation
frames. The worker merges and forwards them immediately, coalesces full-history saves
with one transaction in flight, and acknowledges the latest included serial
after the strict IndexedDB transaction commits. Local save status stays pending
while newer changes remain uncommitted. Incoming
network frames merge in the worker and are forwarded to the synchronous replica,
where they merge with any newer local work. Worker text snapshots never replace
unacknowledged local edits. Position changes are applied incrementally; stable
CRDT cursor anchors resolve after remote merges.

IndexedDB stores one full-history snapshot per room/writer session. Loading joins
all saved sessions before opening a fresh writer, so independently offline tabs
do not overwrite one another's state. This intentionally retains historical
snapshots; storage growth and safe compaction remain future work. A save status
is shown only for the latest local serial acknowledged after transaction commit.

A room is capped at 50 sessions. Up to eight use a full mesh; larger groups
use the bounded overlay described in [browser-overlay.md](browser-overlay.md). Deterministic offer initiation avoids offer
collisions, early ICE candidates are queued, document channels are reliable and
ordered, and presence uses an unordered channel with no retransmissions. Queues
are bounded; dropped immediate sends remain recoverable from retained history.
Signaling relays validated SDP/ICE metadata and never document frames. Healthy
WebRTC connections remain open when a signaling session disconnects.

The current browser room uses fixed membership timing. Real-device, background
tab and WAN measurements are still required before tuning those defaults.

IME composition defers remote painting. On composition end the worker supplies
its retained history, which merges with any newer synchronous local edits. This
avoids interrupting composition while preventing an unbounded deferred-frame
queue. Generic editor/browser undo shortcuts are disabled until collaborative
undo is designed. Presence labels are decoration metadata, not document text.

Browser checks now include eight contexts, explicit offline editing on both
sides, a signaling outage, forced local TURN (selected candidate verified),
Chromium IME events, large paste/backup restore, and production service-worker
offline reload. OS-level composition and separate physical networks still need
manual verification.

The browser full mesh uses source-only broadcast to its at most seven direct
neighbors for live edits. This avoids the tail of probabilistic dissemination
waiting for a one-second repair interval in a small foreground room. Receivers
do not re-forward those live copies. Native larger groups retain two-neighbor
gossip. Both modes share CRDT semantics, batching, snapshots, causal receipt and
anti-entropy; the policy is an explicit adapter configuration.

The larger-room adapter streams reliable document frames into the editor
replica, caches rendered text by immutable history length, and avoids painting
control packets or rebuilding unchanged cursor decorations. Network and local
editor transfer buffers are separate. Single-frame messages bypass incomplete
transfer slots; fragmented summaries use reliable channels.
