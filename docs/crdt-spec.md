# ConV sequence CRDT — initial specification

Status: draft. This specifies a clarity-first reference model, not a complete
wire protocol or a production implementation. Review and property testing must
precede an optimized document implementation.

## Identity and state

A writer session has a fresh random 128-bit replica ID, allocated by its host.
Each transaction receives a contiguous, checked, per-writer mutation sequence
starting at one. Each inserted Unicode scalar has an element ID consisting of
(replica ID, mutation sequence, zero-based scalar offset). Deletes also consume
mutation sequences; element records alone are not a causal summary.

State contains immutable insertion records indexed by element ID and a grow-only
set of deleted element IDs. Each record contains one Rust char, an optional
predecessor element ID (None is the root), and ordering metadata. A fragment can
contain missing predecessors and delete targets. Retain those dependencies.

The host maintains a checked Lamport clock, advancing past observed timestamps
before generating each local transaction. All records in that transaction share
its timestamp. Chained scalar offsets establish order within an inserted run.
The clock is ordering metadata, never evidence that missing mutations arrived.

## Merge and validity

Merge unions records and deletion markers. A repeated ID must refer to exactly
the same immutable record. Conflicting IDs are an error; reject the merge before
modifying state. On valid compatible states, union is associative, commutative,
and idempotent. Canonical equality includes invisible records and tombstones.

Locally generated anchors must already be known. Each insertion must point to
root or an element with an earlier transaction timestamp, or an earlier offset
in its own transaction. This forbids cycles. A receiver retains unresolved
records and validates these relationships when dependencies arrive. A self
anchor and an invalid parent order are rejected atomically. Limits, wire
validation, and writer allocation are not implemented in the reference yet.

## Sequence order

Traverse from root depth first. Visit siblings in descending lexicographic order
of (logical time, replica ID, mutation sequence, offset). Emit each visited scalar
unless deleted, then visit its children even when it is deleted. Unreachable
records remain stored but invisible. An explicit traversal stack avoids recursion
on long documents. The reference uses repeated scans for simplicity.

For existing a -> b with timestamps 1 and 2, inserting x after a at timestamp 3
visits x before b: axb. Inserting xy chains y after x, yielding axyb.
Two concurrent runs AB and XY inserted at the same anchor are ordered by their
first elements' deterministic keys, yielding ABXY or XYAB in that simple case.
This does not promise author-intent preservation for every concurrent edit.
Deleting a hides a without hiding its descendants. A deletion received before
its insertion still hides the element when it eventually arrives.

## Editor and persistence boundary

Positions in this model count Unicode scalars. The future CodeMirror adapter
must map UTF-16 offsets explicitly, preserve stable selection anchors, and tag
remote transactions. Combining marks remain separate elements. Restore existing
IDs, but start a new writer session on every restore/tab. Persist document state,
mutation history, causal summaries, and pending local changes atomically.

## Next verification gates

Add exhaustive small-state and seeded property checks for merge algebra,
permutations, batching, duplicate fragments, conflicting IDs, and future edits
following convergence. Keep regression seeds. Then add a separate indexed
implementation for differential checks, versioned serialization, contiguous
mutation summaries with holes, and a deterministic two-peer protocol harness.

## Implementation status

`Document` now indexes children waiting for missing predecessors. Incoming
records are validated against both existing and incoming state; only previously
unresolved edges affected by those records need rechecking. No state changes
until every new/resolved edge validates. The derived pending index is rebuilt on
decode and participates in state equality checks.

`reference::ReferenceDocument` retains full-union validation and repeated scans
for independent differential checks. One hundred seeded histories compare both
models after reordered and duplicate deliveries, including metadata and visible
text. This supplements exhaustive small-state merge-law tests.

Editor replacements use UTF-16 boundaries and split large inserts/deletes into
complete mutations of at most 4,096 scalar elements/targets. Snapshot bootstrap
stages all received mutations and joins them atomically with current local state.
