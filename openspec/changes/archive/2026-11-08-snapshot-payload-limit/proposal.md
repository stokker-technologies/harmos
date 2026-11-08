## Why

A consumer-owned total snapshot bound prevents recovery checkpoints as state and history grow. Preserve undo history and the recorded tail because a durable checkpoint permits journal compaction.

## What Changes

- Interpret SnapshotPolicy.max_bytes as a per-segment bound; keep all budgets unchanged.
- Spool ordered snapshot bytes in bounded segments and carry a manifest with logical position, count, total bytes, and SHA-256 per segment.
- Publish a filesystem snapshot atomically only after all segments and manifest are durable; retain the prior snapshot and journal on interrupted writes.
- Report oversized history entries with their size, bound, and replay consequence.

## Capabilities

### New Capabilities
- `segmented-state-snapshots`: Bounded checkpoint segments and verified recovery.

### Modified Capabilities

## Impact

Owned scope: harmos-storage snapshot types, admission, readers and file backend; harmos-runtime snapshot codec, capture and recovery; affected storage/runtime fixtures and focused tests; README and product guide. Journal records and durability are unchanged. Dokime migration is explicitly separate and not edited here.
