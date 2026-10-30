# ConV wire protocol v1

Messages use Serde/Postcard. Each payload is an envelope containing version 1
and a Hello, membership/probe, Summary/SummaryReply, Request, Mutation/Batch,
or snapshot offer/request/state message. Documents are decoded through validated
insertion records and deletion markers. Summaries must be canonical.

Every datagram is at most 1,200 bytes. Header (little endian): magic `CV01`
(4 bytes), room ID (16), sender session ID (16), message ID (8), chunk count (4),
zero-based chunk index (4), and CRC32 of the complete payload (4). The remaining
1–1,144 bytes are payload. Every chunk except the last is full-sized.
Sender sessions allocate unique increasing message IDs. Mutation IDs remain
separate from message IDs.

Reassembly limits: eight incomplete messages, 8 MiB of received payload, and a
ten-second expiry using host-provided monotonic time. Invalid rooms, versions,
counts, lengths, checksums, and inconsistent duplicate chunks are rejected.
No mutation is acknowledged until the entire message decodes and merges.
Checksums provide corruption detection, not authentication.

Periodic summaries retry missing mutations after a dropped message or incomplete
transfer. Incomplete messages are restarted through retained-range repair;
targeted missing-chunk retransmission is not used. Summaries, mutations and
snapshots share the 8 MiB message cap. Oversized bootstrap snapshots fall back
to bounded mutation-range repair. These are explicit resource limits.

Snapshot offers are followed by an explicit request and a complete retained-
history snapshot. Snapshot messages use the same bounded chunk reassembler;
installing one stages and merges all mutations with existing local edits. A lost
snapshot falls back to periodic range repair, which retransmits complete
missing mutation messages. A first chunk never acknowledges the whole transfer.

Native gossip mode forwards each newly accepted mutation once, to at most two peers.
Browser rooms of at most eight members broadcast from the source to their at
most seven links and suppress forwarding. Larger rooms forward every newly
accepted mutation over at most eight current overlay links, excluding the
immediate sender. Mutation IDs suppress loops. Steady overlay degree is six.
Periodic summary exchanges are bidirectional (Summary/SummaryReply) without an
unbounded echo loop. Hosts batch small mutations for 20 ms in a bounded outbox
(256 queued items, 8 MiB serialized mutation bytes); batches hold at most 32
mutations and normally target sub-frame payloads. Discarded immediate gossip
remains in retained history. Control messages bypass the outbox; small browser control uses a separate
unordered channel with no retransmissions. Fragmented control uses the reliable
channel to avoid losing pieces of causal summaries. Bulk document data
uses reliable ordered channels. Both channels share WebRTC congestion resources.

Declared sequence/map counts are checked before reserving decoder storage.
Mutation records/deletion targets are capped at 65,536 each, range requests at
64, snapshots at 100,000 mutations, and backups at 1,000,000 mutations/64 MiB.
Sparse summary entries and historical writer counts are each capped at 65,536.
These admission limits reject excess input without discarding integrated state.

Complete one-frame messages need no incomplete-transfer slots, even when bulk
transfers fill the reassembler. A conflicting fragment count for an outstanding
message ID is still rejected. Browser loopback delivery from the editor has a
separate bounded reassembler so network pressure cannot reject local work.
Overlay control routing uses a bounded JSON wrapper (up to 8,192 characters,
1,200 frame bytes and 16 hops). Presence wrappers are capped at 4,096 characters
and 128 anchor bytes. Origin/sequence deduplication retains at most 8,192 keys.
Both wrappers travel only over WebRTC; signaling carries neither.
