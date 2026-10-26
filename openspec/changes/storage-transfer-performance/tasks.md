# Tasks

## 1. Benchmark Contracts And Baseline

- [x] 1.1 Define explicit reference configuration, workload shapes, ownership and measurement contracts in design.md; review against consumer-policy guidance.
- [x] 1.2 Rework storage benchmarks and shared fixture settings for bounded 30 MiB uploads/downloads, 10,000 records and 1,000,000 samples with symmetric writes/reads, plus concurrent files; verify compiled benchmark enumeration and correctness assertions.
- [x] 1.3 Rework real sidecar, guest and external gRPC benchmark categories, canonical fixture preparation and Cargo targets; preserve relevant existing kernel coverage and verify real boundary execution.
- [x] 1.4 Establish the shipped JsonStorage baseline after the authorized backend replacement once the machine is ready for measurements; preserve native Criterion evidence and disclose that no old-backend baseline exists.

- [x] 1.5 Add shared typed transfer fixtures for real sidecar and guest boundaries: 10,000 records, 30 MiB file bytes, and 1,000,000 samples in each direction, bounded batches, completion acknowledgments, ordering/count/content verification, and matching benchmark-feature builds of shipped Schematic examples, unary batches beside streams. Keep startup and command latency supplementary.

## 2. Owned Storage And Scheduling

- [x] 2.0 Replace mixed atomic backend writes with owned independent typed durable writes, migrate every implementer, and verify partial-success/retry/cursor behavior.
- [x] 2.6 Ship thin JsonStorage with bounded disk reads, typed durable persistence, raw file lifecycle, historical snapshots and recovery metadata; reuse FileBackend for state and verify real persistence/reopen stories.
- [x] 2.7 Switch ordinary persistence stories and storage benchmarks to shipped JsonStorage; retain controlled fault backends only for fault-specific boundaries and remove superseded JSON test persistence.
- [x] 2.1 Migrate owned records, streams, objects and projected-record admission across domain/storage/runtime and all tests, benches and examples; targeted ownership/correctness proof.
- [x] 2.2 Migrate owned file chunks and backend attempt ownership without relocation; preserve retry identity, hashes, cancellation and publication with targeted file stories.
- [x] 2.3 Remove recovery admission/batching/blocking-adapter deep copies and IPC admission/output/sizing copies; targeted recovery and Arrow retention stories.
- [x] 2.4 Add explicit bounded independent file I/O, per-file ordering and supervised shutdown while preserving serialized cursor barriers and conservative backend failure gating; targeted concurrent/failure stories.
- [x] 2.5 Update affected README/product-guide contracts and consumer examples together with API changes; remove superseded internal interfaces.

- [x] 2.8 Remove typed-row copies in attachment batch selection and raw recovery redelivery; migrate runtime attachment consumers and Schematic journal/stream adapters to borrowed inspection or shared immutable ownership, preserving exact persisted bytes and fan-out.

- [x] 2.9 Remove per-byte checksum formatting allocations from JSON persistence and reads while preserving the on-disk format, entry limits and corruption detection.

- [x] 2.10 Persist shared production checkpoints without metadata deep copies and verify only newly appended bytes from the durable hash state; preserve append-only identity, checkpoint replay and corruption rejection with targeted JSON stories.

- [x] 2.11 Borrow raw managed-journal JSON payloads for transaction decoding and skip unknown record payload trees, preserving snapshot decoding and recognized Prepared payloads; verify recovery behavior without claiming a universal budget for consumer-owned application state.

- [x] 2.12 Add unique consuming ReadBatch projection with preallocation peak admission, retained claims, typed conversion errors and fail-fast saturation behavior; verify ownership, alias, failure and concurrent-admission invariants and hand the usable interface to Dokime.

- [x] 2.14 Provide caller-scoped admission for composed typed readers, object point reads and bounded projected-page collections; preserve the shared dependency group across retries, refuse unavailable grouped query slots without self-wait, and prove saturation and retained allocation lifetimes.

- [x] 2.13 Review remaining coordinated Dokime constraints for backend write workspace, opened-file identity, transport framing allowances and Value conversion; distinguish existing facilities from new contracts, agree any required changes and preserve consumer safeguards until replacements are verified.

- [x] 2.15 Add consuming two-input ReadBatch projection for owned composed results; admit caller scratch and internal guard-vector growth before allocation, preserve every input claim, reject aliases and overflow, and prove sequential folds and failure lifetimes.

- [x] 2.16 Extend composed read admission to Arrow queries, stored/local file readers and nonblocking mutable metadata claims; provide retained consuming Arrow/file DTO projection, preserve source claims until IPC destination admission, account private scaffolds and alias-safe growth, and verify failure/lifetime behavior before rebuilding benchmark artifacts.

- [x] 2.17 Expose a passive saved-cursor wait using existing storage change notifications; verify no snapshot-registration wakeup gap, cancellation, coalescing, history validation and terminal close/error behavior without persistence or retry side effects, and document the durable visibility boundary.

- [x] 2.18 Expose Arrow payload encoding preallocation bounds by reusing the existing counting encoder; include output collection/row storage, document staged retained encoding then typed decoding, and verify supported encodings, malformed data and limit behavior without guessed memory factors.

- [x] 2.19 Review and close producer receipt-response ownership before accepted writes: use authoritative preallocation bounds or a retained response contract as required, preserve cursor ownership through retry/cancellation, and avoid fallible post-write admission.

## 3. Transport And Accounting

- [x] 3.1 Make primitive buffer memory accounting independent of element count while preserving nested allocation/saturation correctness; focused domain proof.
- [x] 3.2 Remove redundant gRPC sizing/encoding and decoding intermediate work without changing wire formats or bounded validation; targeted codec and stream proof.
- [x] 3.3 Preserve sidecar payload allocation across framing and decoding where possible, retaining credits and cancellation ordering; read the length prefix without an unnecessary first-byte handoff, preserving fragmented reads and EOF; targeted carrier proof.
- [x] 3.4 Assess guest measurements and implement justified ownership/codec improvements, preserving component isolation and updating carrier fixtures; report measured limits rather than promising unsupported zero-copy.

- [x] 3.5 Replace sidecar request/reply envelope encode-for-sizing with borrowed counting-only sizing; include actual portable context in child-call fit admission and verify equality to encoded frames at boundary sizes.

## 5. Unified Filesystem Operations

- [x] 5.1 Replace stored-only ReadFile with FileSource::Stored(FileReference) | Local(PathBuf), shared range semantics and lifetime-charged FileReader batches; migrate domain codecs, storage engine, runtime handlers, all callers and retained-slice tests.
- [x] 5.2 Implement HashFile with source/range, bytes_read and SHA-256 using bounded charged streaming buffers; implement CaptureFile with metadata and explicit consistency, independent durable stored output, in-transfer hashing and detected-mutation rejection without claiming atomic snapshots.
- [x] 5.3 Replace FollowFile with source/start and charged data plus explicit truncation/replacement events; retain necessary staged-production following under its distinct production contract, migrate consumers, own polling/watch/cancellation and test termination and changes.
- [x] 5.4 Implement ListDirectory and WatchDirectory with shared DirectorySelection, ordered case-sensitive slash-relative include/exclude globs, recursive traversal without symlinks, coherent initial enumeration and explicit overflow/rescan events; test arbitrary accessible paths, later descendant includes and rename/change races.
- [x] 5.5 Expose all new operations through shared domain catalogs, native runtime, gRPC and plugin/guest calls without operation-specific carrier exclusions or extra per-plugin allowlists; allow any host-accessible filesystem path without configured-root boundaries, with targeted native/gRPC dispatch proof and shared plugin/guest bindings.
- [x] 5.6 Compose capture/replay/state and filesystem settings under existing owners; replace superseded read/tail/snapshot limits, retain necessary bounds, and verify lifetime accounting for native copies, hashes, retained file slices and typed-record pages without double charging.
- [x] 5.7 Add applicable read/hash/capture/directory/follow benchmarks using bounded datasets and correctness checks; keep existing comparison workloads unchanged, with final execution tracked in 4.2.
- [x] 5.8 Update shipped examples, public documentation and generated consumers; remove superseded operations/types/files and complete targeted filesystem correctness proof.

## 4. Integration And Measurement

- [x] 4.7 Instrument newly introduced storage, retained-read/projection, JSON, filesystem and carrier paths using existing tracing conventions; preserve request context across spawned/blocking work and cancellation, exclude payloads, and verify representative trace lifetimes and disabled-tracing behavior.

- [x] 4.6 Integrate current dev observability instrumentation with owned storage and carrier APIs, resolve source/tooling overlaps, and complete targeted merge proof before publication.

- [x] 4.1 Review all writer handoffs and combined migration inventory; complete targeted checks and close every implementation item before declaring source complete.
- [x] 4.2 Run one final combined measurement after source closure: rerun identical comparison workloads and new filesystem workloads, execute the untimed real sidecar/guest Hash bridge correctness checks, compare latency/throughput/memory with native baseline results and repair regressions; label new workloads without baselines and report achieved and missed targets truthfully.
- [x] 4.3 Present complete candidate and request remote delivery authorization unless subsequently granted.
- [x] 4.5 Once the complete implementation is verified better than or comparable to its existing benchmark baselines, message the Dokime storage optimization chat to adopt the new Harmos code and run its own benchmarks; identify the actual available revision/worktree and measurement evidence without implying unpublished code is released.
- [ ] 4.4 After authorized push, verify remote revision, start required all-platform local CI simulation alongside hosted checks, enable ready review auto-merge and report actual proof; no monitoring.

## Workflow Follow-Up

- Finish/archive the specification only after tracked work and required proof are complete; retire workspace only through canonical housekeeping.
