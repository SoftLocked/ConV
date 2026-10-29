# ConV wire protocol v1

Messages use Serde/Postcard. Each payload is an envelope containing version 1
and a Summary, Request, or Mutation. Documents are decoded through validated
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
transfer. Targeted missing-chunk retransmission and snapshot bootstrap are still
pending. Large summaries and individual mutations currently share the 8 MiB
message cap. Limits are prototype defaults and must be tested under load.
