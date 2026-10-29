# filesystem-operations Specification

## Purpose
Defines the filesystem operations Harmos ships: unified file sources, hashing and capture, file following, directory selection and watching, under composed resource policy and measurement.

## Requirements

### Requirement: Unified File Sources
The system SHALL expose stored and executing-host local files through one FileSource and ReadFile source/range contract. Reads SHALL stream bounded data whose backing allocations survive retained aliases.

#### Scenario: Shared Local Access
- **WHEN** a native caller, gRPC client, sidecar or guest invokes a local-file operation at any path accessible to the executing host process
- **THEN** the same implementation SHALL execute it without a carrier-specific exclusion or additional per-plugin allowlist, subject to ordinary operation authorization and host filesystem permissions

#### Scenario: Arbitrary Accessible Paths
- **WHEN** a caller requests an accessible local path outside the application workspace
- **THEN** the operation SHALL allow it without a configured-root boundary or path allowlist

#### Scenario: Symlink Traversal
- **WHEN** a requested path traverses a symlink
- **THEN** the operation SHALL reject it without reading or capturing the target

### Requirement: Hash And Capture
HashFile SHALL return bytes_read and SHA-256 for the selected source/range using bounded buffers. CaptureFile SHALL create an independent immutable durable FileReference and hash during transfer. Stable-source capture SHALL reject detected mutation and SHALL NOT claim an atomic snapshot guarantee.

#### Scenario: Independent Capture
- **WHEN** a local range is captured successfully and the local source is subsequently changed
- **THEN** the stored file SHALL retain the captured bytes and matching digest

#### Scenario: Source Mutation
- **WHEN** capture detects source replacement, truncation or modification under stable-source consistency
- **THEN** it SHALL fail without publishing a successful immutable reference

### Requirement: File Following
FollowFile SHALL accept Beginning, End or At(offset), emit bounded data and explicit truncation/replacement events, and own cancellation and watch/polling lifetimes. Existing staged-production following SHALL remain a distinct supported capability with migrated callers.

#### Scenario: Follow Changes And Cancel
- **WHEN** a followed local file grows, truncates or is replaced
- **THEN** the reader SHALL deliver available bytes and the corresponding change events without silently treating replacement as continuation, and cancellation SHALL release resources

### Requirement: Directory Selection And Watching
ListDirectory and WatchDirectory SHALL share root, recursion and ordered include/exclude rules. Rules SHALL match case-sensitive slash-relative paths with star confined to a segment, double-star crossing segments, last match winning and unmatched paths excluded. Traversal SHALL not prune excluded ancestors when descendants can match and SHALL not follow symlinks.

#### Scenario: Later Descendant Inclusion
- **WHEN** an excluded directory contains a descendant matched by a later include rule
- **THEN** the descendant SHALL still be discovered

#### Scenario: Initial Scan And Overflow
- **WHEN** watching begins with include_existing enabled while files change
- **THEN** Existing and InitialScanComplete SHALL coordinate with subsequent Created, Modified, Removed and Renamed events, and uncertainty or notification overflow SHALL produce RescanRequired rather than silent loss

### Requirement: Composed Resource Policy And Measurement
The system SHALL compose filesystem and capture/replay/state resource settings under their existing owners, retain ordinary shared data ownership without restoring the superseded storage read pool, and measure applicable hash/capture/read/follow/directory workloads after the combined implementation. New workloads SHALL be identified as having no historical baseline.

#### Scenario: Retained Buffers
- **WHEN** a consumer retains a file slice or typed-record page after the producer advances or closes
- **THEN** its backing allocation SHALL remain valid until the final owning alias is released
