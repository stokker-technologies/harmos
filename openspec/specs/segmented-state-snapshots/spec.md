# Segmented State Snapshots Specification

## Purpose
Preserve complete managed-state recovery as checkpoints grow beyond a consumer-owned segment bound, with verified ordered segments and atomic durable publication.

## Requirements

### Requirement: Segmented Checkpoints
The runtime SHALL preserve state, undo history, receipts and recorded tail in ordered payload segments no larger than SnapshotPolicy.max_bytes. The manifest SHALL identify logical position, segment count, total bytes and each segment SHA-256. Recovery SHALL verify every segment before applying the snapshot.

#### Scenario: Large State Recovery
- **WHEN** a state exceeds several payload segments
- **THEN** snapshot plus subsequent journal recovery equals full journal replay

#### Scenario: Corrupt Segment
- **WHEN** a segment is missing, reordered or modified
- **THEN** recovery rejects the snapshot before exposing its state

### Requirement: Atomic Publication
Backends SHALL publish only complete durable snapshots and SHALL delete covered journal entries only after publication. Journal format and durability SHALL remain unchanged.

#### Scenario: Interrupted Snapshot
- **WHEN** persistence stops between segments
- **THEN** reopening uses the previous snapshot and retained journal

### Requirement: Actionable Refusal
A snapshot history entry larger than the segment bound SHALL be refused with its encoded byte size, segment limit and number of journal entries requiring replay. Other failures SHALL report available size information and replay consequence.

#### Scenario: Oversized Entry
- **WHEN** an undo or recorded entry exceeds the segment bound
- **THEN** capture reports the size, bound and next-boot replay count and preserves recovery data
