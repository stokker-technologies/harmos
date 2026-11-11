## Decision

Use a file-backed snapshot payload. The serializer writes a byte stream into segments no larger than the consumer's existing `SnapshotPolicy.max_bytes`; it does not retain a whole-snapshot byte vector. Application state can cross segment boundaries. Undo, receipt and recorded-tail entries are measured individually and refused if they exceed the segment bound. No configured budget is increased.

The payload owns its temporary spool and ordered manifest. Clones share the immutable spool, and readers open independent read-only cursors. The manifest records logical position, count, total bytes, lengths and SHA-256 hashes. A digest over ordered length/hash descriptors binds segment ordering.

## Persistence

The filesystem checkpoint is a manifest header, segment frames, and a matching commit manifest. Segment frames use the existing integrity-envelope codec with fixed-width raw-payload digests; the journal record format does not change. `max_checkpoint_bytes` applies to each encoded segment frame. Fixed-size manifest records use a protocol metadata bound.

Write the complete checkpoint to the existing pending path, flush and sync it, then atomically replace the published checkpoint. Journal compaction remains strictly after successful publication. A failed write between frames leaves the old checkpoint and journal available. Readers verify count, total bytes, ordered descriptor digest, each segment digest, matching commit manifest and end-of-file before exposing a snapshot.

Existing persisted single-payload checkpoints remain readable. This is necessary durability compatibility, not a temporary writing adapter: new captures always use segments. The imported legacy payload has no separate manifest position; runtime recovery retains its existing payload-position and journal-boundary validation.

## Diagnostics

The journal writer retains its startup tracing dispatcher through final capture, so shutdown warnings remain observable with scoped subscribers. Capture records its last snapshot frontier only after persistence completes. Replay counts use journal group sequences, not logical positions. Oversized entries report their exact encoded size and the configured segment bound. Other failures report known payload size or explicitly mark size unavailable. Undo history, receipts and the tail are retained.

## Ownership

This change owns Harmos runtime and storage snapshot producers and consumers, their focused tests, and current documentation. Dokime migration is scheduled separately by the owner and is not part of this checkout.
