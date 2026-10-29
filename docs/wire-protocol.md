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
The eight-peer browser full mesh broadcasts from the source to its at most seven
links and suppresses duplicate forwarding.
Periodic summary exchanges are bidirectional (Summary/SummaryReply) without an
unbounded echo loop. Hosts batch small mutations for 20 ms in a bounded outbox
(256 queued items, 8 MiB serialized mutation bytes); batches hold at most 32
mutations and normally target sub-frame payloads. Discarded immediate gossip
remains in retained history. Control messages bypass the outbox; browser control
uses a separate unordered channel with no retransmissions. Bulk document data
uses reliable ordered channels. Both channels share WebRTC congestion resources.

Declared sequence/map counts are checked before reserving decoder storage.
Mutation records/deletion targets are capped at 65,536 each, range requests at
64, snapshots at 100,000 mutations, and backups at 1,000,000 mutations/64 MiB.
Sparse summary entries and historical writer counts are each capped at 65,536.
These admission limits reject excess input without discarding integrated state.
