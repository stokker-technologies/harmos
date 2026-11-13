# Coupled Storage Ownership

## Ownership

The paired consumer owns its adoption and published dependency pin. No local override or path-resolved lockfile enters delivery.

## Consumer Evidence

Dokime's native output capture retains opened handles and resumable producers; a path-only CaptureFile loses original-object identity and exact retry state. Accepted append calls can outlive their initiating caller, so retained data must belong to actual accepted work.

## Design Constraints

The locked prototype-streamline Slice H in task 1.17 supersedes the original ReadSession, allocation-claim and backend-scratch admission design. StorageManager and ordinary shared batches own data through blocking work, retries and cancellation. Consumer settings bound serialized queues/pages and query concurrency; consumers own retained-result and conversion-scratch budgets. Do not restore read pools, scratch carriers or a second accounting mechanism.

Opened native file handles are process-local capabilities, not serialized path substitutes. Capture into a retained producer must expose acknowledged progress and permit exact retries without silently replacing sources or targets. Keep portable wire contracts distinct from native capabilities.

### Windows Opened-File Cursor Contract

Opened-file reads preserve independent caller cursors and the original filesystem object. On Windows, `FileExt::seek_read` changes the file cursor shared by duplicated handles, so native readers use `ReOpenFile` to create an independent same-object read handle. This requires the supplied handle to allow a subsequent read open; exclusive handles such as `tempfile::tempfile()` are rejected with an explicit invalid-input error. The independent reader requests read access and permits read, write and delete sharing so delete-on-close sources remain compatible. It never resolves or reopens a source path.

The native `FrozenFile` backing retains its private named temporary until its readers, captures and blocking workers release it. Do not unlink it before a Windows independent reopen: corporate filesystem filters can deny reopening deleted-while-open temporaries. Native coverage retains named source owners and verifies independent cloned cursors, snapshot reads/capture after source removal and refusal of an exclusive tempfile handle. Dokime temporary producers must likewise keep named or shareable delete-on-close owners rather than calling `NamedTempFile::into_file()` before native reads. Local filesystem failures include the failing step and local path, preserve the native error source, and use a diagnostic handle-path lookup without reopening by that path. Dokime's opened/frozen consumers that use anonymous tempfiles are `runtime/storage/{artifacts,native_outputs}.rs` and `server/grpc/bundle.rs`; the ZIP and JSON bundle tempfiles are read through the opened-file API, and the empty output snapshot is frozen there.

Backend workers retain their actual input/output owners through blocking work. Request ownership follows accepted request retention, including cancellation after acceptance; consumer scratch budgets remain consumer-owned.

The native opened-capture constructor synchronously retains the original file and target. Fallible reader setup and copying occur in `OpenedCapture::finish(&mut self)`, whose retained future and exact chunk survive caller cancellation. Initialization failure must not abandon the producer and recreate its identity on retry. Existing target prefixes are compared before any new suffix is accepted. Read batches and retained append chunks share their ordinary backing allocations; uniquely owned projections move data without copying, and aliases refuse consuming typed/file projections.

Batch writes use shared StorageBatch ownership without a scratch-admission carrier. Accepted workers settle during shutdown while new admission stays closed. RequestClaims carries process-local ownership through Host::call_owned and accepted runtime requests. Cross-process adapters refuse such owners rather than dropping or serializing them.

## Integration and Validation

The Dokime manifest keeps its published dependency while an untracked local Cargo patch selects all required Harmos packages from this worktree. Verify the resolved graph before consumer checks. Do not commit the override or local-path lockfile resolution. Publish/pin/remove-override remains a separate future authorized delivery step.

Use project-task compiler preflight. The read-path follow-up authorizes focused executed tests and timing diagnostics without timing assertions. Other consumer integration and final validation tasks remain open. Full agreed source, including consumer removal, must be ready before the prototype build. File polling is allowed under the shared Harmos filesystem policy.

## Read-Path Decisions

Decode validated Postcard events directly into Arrow column builders, preserving optional parents, lists, variants, values and row order. Push an initial payload-only projection into decoding while validating the complete payload, including unselected fields. Keep the previous row-tree implementation only as the requested equivalence oracle in tests.

Keep internal scans at 16,384 rows / 8 MiB. Query admission concurrency and response sizes remain explicit consumer settings. Cache at most 32 MiB of compact Arrow input pages and 32 MiB of coordinates, spilling above those bounds; external sort runs stay disk-backed so the number of runs cannot multiply retained memory. Exact LTTB, extrema and decimation selection remain unchanged.

The JSON backend and its timing harness are removed by `facility-trim`. Consumer-backend
measurements must establish end-to-end projection and bounded-window latency; a decoder
microbenchmark does not prove the consumer's one-million-sample acceptance. Backend implementation
and the consumer's Arrow-to-Postcard conversion belong to the consumer. Do not restore JsonStorage
or JSON-only fixtures to complete earlier measurement tasks.

## Publication Decision

Retain existing Harmos delivery rings and authored per-stream window sizes, as the user explicitly selected. Typed publication remains infallible. Reverse the abandoned central stream-policy and window-removal changes while preserving unrelated native storage work. Dokime live plots remain storage-authoritative; saved-frontier notifications serve those observers without replacing the pre-storage payload queue or SDK listener delivery.

## Immutable Mutable-Source Snapshots

Freeze an opened source into a private native-owned temporary file before backend creation or capture waits. The immutable FrozenFile exposes length and SHA-256 but no writable handle; its owner survives backend retries even if the original producer changes. StorageManager::freeze_opened_file reuses bounded opened-file reads and verifies source stability. StorageManager::capture_frozen_file retains that owner inside existing OpenedCapture. Partial freezes may be discarded on cancellation, with in-flight blocking IO retaining its actual file and batch owners until completion. Disk errors propagate; no replacement arbitrary total-size ceiling is introduced. Dokime retains digest-based deduplication and removes its duplicate copy/hash loop.

## Current Source Inventory

| Work | Harmos Ownership |
| --- | --- |
| Opened/frozen identity and follow/capture | Storage `local.rs`, `frozen_file.rs`, `opened_capture.rs`, `file_reads.rs`, `file_follow.rs`, `file_production.rs`, directory modules and manager/files fixtures |
| Retained writes and admission | Storage backend/batch/read-batch, staging/producer and mutation modules, receipts, queue and manager lifecycle; runtime append-file and request ownership |
| Read/query work | Storage `query/`, `arrow/`, `statistics/`, `state/file/`, config and backend-neutral integration fixtures |
| Transactions | Runtime state manager/writer/envelope/history/recovery/snapshot codec and storage attachments; Schematic constructors and ownership fixtures |
| Schema settings | Runtime setup/state/records/streams, operation contracts/registration/construction and native observers; existing record, wire and observer fixtures |
| Framing | SDK grpc count/write traversal and runtime grpc codec/envelope; native preparation fixtures |
| Guidance | Product guide and canonical/generated authoring references; README claims |

The state-file journal builds a rebuildable temporary offset index on open and updates it after
durable append and compaction. Recovery/retry pages seek directly; cancellation is checked while
indexing. Only recovery.open uses its existing extended policy. JSON data-backend indexing and
persistence fixtures are removed; the independent state-file journal remains.
Queries use bounded private Arrow spill, stable external merge, grouped reduction, exact
order-statistic selection and coordinate passes. The query slot survives through reduction and
output; dropped callers cancel detached blocking work. Journal flushes use bounded chunks with
unchanged durable acknowledgment and cadence. Listener catch-up uses bounded row/byte pages and
yields between pages. Validate fairness and working-set peaks separately from correctness.

Native transaction submission retains `Response<A::Transaction>` through the queue, recorded
history, undo/replay and captures. `commit_owned` and `commit_keyed_owned` accept that owner;
ordinary generic wrappers preserve their signatures and serialized enum.
Native schema preparation shares the configured immutable SchemaSettings snapshot across
records, streams, operations and native observers, while explicit default wrappers stay available.
Parameterized frame allowances use schema/member identity and selected variants in the canonical
count/write traversal. Prepared frames retain their encoded owner and are sent without re-encoding.

## Storage Deletion and Windows Follow-Up

In `crates/harmos-runtime/tests/storage/operation_deletion.rs`, `deletion_preserves_other`
expects a reader opened before deletion to retain its bytes and fresh reads to fail. Its fixture
uses the in-memory backend; that assertion alone establishes no consumer-backend guarantee.
Review the intended deletion contract against backend-neutral storage ownership before treating
this test repair as complete proof.
Named temporary owners remain alive through reads and blocking workers.
## Open Points

### Awaiting Owner Confirmation

- Accept retained pre-deletion reader bytes, with fresh reads refused, as the intended backend-neutral contract after reviewing consumer backends.

The consumer's one-million-sample latency acceptance and native Linux proof remain open;
no removed JSON benchmark supplies either. The numerical consumer acceptance target must be
recovered from its owning specification before a new measurement is judged.
