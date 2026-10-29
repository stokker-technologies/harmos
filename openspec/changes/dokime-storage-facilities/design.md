# Coupled Storage Ownership

## Ownership

The upstream integration worktree is `harmos.wt/dokime-storage-facilities`, branch `feat/dokime-storage-facilities`. Its consumer is `dokime.wt/storage-optimization`, branch `perf/storage-optimization`, OpenSpec change `storage-optimization`. The parent chat owns both specifications and validation; assigned workers receive disjoint source inventories. No cross-chat coordination or publication is needed for local integration.

## Consumer Evidence

Dokime's native output capture retains opened handles and resumable producers; replacing them with a path-only CaptureFile loses original-inode identity or exact retry state. Existing FollowFile and production following create private admission groups, preventing safe composition with retained metadata under one ReadSession. SQLite receives owned input batches but no writable scratch admission carrier for new row/index/receipt serialization. Portable append calls can retain copied request data after their initiating caller is cancelled, so a caller-local claim is insufficient.

## Design Constraints

Extend the existing facilities instead of adding a parallel accounting system or consumer-specific worker. Additional allocations must be admitted before construction, including simultaneous source/target peaks. Claims stay with actual retained data through cancellation and terminal consumption. Dependent operations use the same session and nonblocking growth so a retained source cannot deadlock an unrelated capacity wait. Preserve zero-additional-allocation moves at saturation.

Opened native file handles are process-local capabilities, not serialized path substitutes. Capture into a retained producer must expose acknowledged progress and permit exact retries without silently replacing sources or targets. Keep portable wire contracts distinct from native capabilities.

### Windows Opened-File Cursor Contract

Opened-file reads preserve independent caller cursors and the original filesystem object. On Windows, `FileExt::seek_read` changes the file cursor shared by duplicated handles, so native readers use `ReOpenFile` to create an independent same-object read handle. This requires the supplied handle to allow a subsequent read open; exclusive handles such as `tempfile::tempfile()` are rejected with an explicit invalid-input error. The independent reader requests read access and permits read, write and delete sharing so delete-on-close sources remain compatible. It never resolves or reopens a source path.

The native `FrozenFile` backing retains its private named temporary until its readers, captures and blocking workers release it. Do not unlink it before a Windows independent reopen: corporate filesystem filters can deny reopening deleted-while-open temporaries. Native coverage retains named source owners and verifies independent cloned cursors, snapshot reads/capture after source removal and refusal of an exclusive tempfile handle. Dokime temporary producers must likewise keep named or shareable delete-on-close owners rather than calling `NamedTempFile::into_file()` before native reads. Local filesystem failures include the failing step and local path, preserve the native error source, and use a diagnostic handle-path lookup without reopening by that path. Dokime's opened/frozen consumers that use anonymous tempfiles are `runtime/storage/{artifacts,native_outputs}.rs` and `server/grpc/bundle.rs`; the ZIP and JSON bundle tempfiles are read through the opened-file API, and the empty output snapshot is frozen there.

Backend scratch must be owned by the native backend call and remain available through its blocking worker; input memory claims alone do not cover new output allocations. Request ownership must follow actual accepted request retention, including cancellation after acceptance.

The native opened-capture constructor synchronously retains the original file and target. Fallible reader setup and copying occur in `OpenedCapture::finish(&mut self)`, whose retained future and exact chunk survive caller cancellation. Initialization failure must not abandon the producer and recreate its identity on retry. Existing target prefixes are compared before any new suffix is accepted. Exclusively owned read allocations transfer atomically into the write pool; aliased allocations cannot shed their original admission.

Batch writes use `StorageBatch::try_scratch` with the existing native write budget. Allocating backend lifecycle and retention calls also receive native scratch, including calls without a batch. Accepted workers retain the claim during shutdown; this does not reopen admission for new work. `RequestClaims` carries process-local ownership through `Host::call_owned` and accepted runtime requests. Cross-process adapters refuse such owners rather than silently dropping or serializing them.

## Integration And Validation

The Dokime manifest keeps its published dependency while an untracked local Cargo patch selects all required Harmos packages from this worktree. Verify the resolved graph before consumer checks. Do not commit the override or local-path lockfile resolution. Publish/pin/remove-override remains a separate future authorized delivery step.

Use project-task compiler preflight. The read-path follow-up authorizes focused executed tests and timing diagnostics without timing assertions. Other consumer integration and final validation tasks remain open. Full agreed source, including consumer removal, must be ready before the prototype build. File polling is allowed under the shared Harmos filesystem policy.

## Read-Path Decisions

Decode validated Postcard events directly into Arrow column builders, preserving optional parents, lists, variants, values and row order. Push an initial payload-only projection into decoding while validating the complete payload, including unselected fields. Keep the previous row-tree implementation only as the requested equivalence oracle in tests.

Keep internal scans at 16,384 rows / 8 MiB. Query admission concurrency and response sizes remain explicit consumer settings. Cache at most 32 MiB of compact Arrow input pages and 32 MiB of coordinates, spilling above those bounds; external sort runs stay disk-backed so the number of runs cannot multiply retained memory. Exact LTTB, extrema and decimation selection remain unchanged.

The native timing harness uses JsonStorage, not Dokime's separately owned SQLite backend. Measure end-to-end reads independently from payload decoding; neither a decoder microbenchmark nor an optimized build proves Dokime's 1M-sample deadline. SQLite implementation and the consumer's Arrow-to-Postcard conversion remain outside this worktree slice.

## Publication Decision

Retain existing Harmos delivery rings and authored per-stream window sizes, as the user explicitly selected. Typed publication remains infallible. Reverse the abandoned central stream-policy and window-removal changes while preserving unrelated native storage work. Dokime live plots remain storage-authoritative; saved-frontier notifications serve those observers without replacing the pre-storage payload queue or SDK listener delivery.

## Immutable Mutable-Source Snapshots

Freeze an opened source into a private native-owned temporary file before backend creation or capture waits. The immutable FrozenFile exposes length and SHA-256 but no writable handle; its owner survives backend retries even if the original producer changes. ReadSession::freeze_opened_file reuses admitted opened-file reads and verifies source stability. StorageManager::capture_frozen_file retains that owner inside existing OpenedCapture. Partial freezes may be discarded on cancellation, with in-flight blocking IO retaining its actual file and batch owners until completion. Disk errors propagate; no replacement arbitrary total-size ceiling is introduced. Dokime retains digest-based deduplication and removes its duplicate copy/hash loop.
