# Design

## Agreed Direction

Payload handoffs consume ownership. Inspection borrows. Retry, replay and Arrow aliases share immutable allocations. The locked prototype-streamline task 1.17 in dokime-storage-facilities supersedes storage allocation claims and read-session admission; configured serialized queue/page bounds and query concurrency remain. Encoding may construct a required representation; intermediate copies require concrete justification. Preserve wire formats, data integrity, durability, ordering, cancellation, mutation identities and bounded admission. The user explicitly chose independent typed backend writes over cross-type atomic transactions.

## Typed Backend And Shipped JSON

StorageBatch<T> owns shared immutable Vec backing and a range; cheap clones/slices retain the allocation. Backend methods write_schemas, write_objects, write_records, write_stream and write_files each accept their typed owned batch and return a durable contiguous cursor. FileChunk is the byte specialization consumed by write_file. Mixed StoragePart/StorageEntry persistence contracts are removed. The manager dispatches in global order; a successful schema write followed by a failed object write may persist only the schema. Retries safely recognize that durable prefix; receipts cannot advance over missing work. Existing state/backend completion remains unchanged in this slice.

JsonStorage is a filesystem adapter, not another scheduler or storage manager. Explicit JsonStorageSettings supplies root and a bounded JSON-entry size. Typed append-only JSON segments retain historical entries without caching all payloads; raw files use staging and final storage. Root leasing, bounded disk reads, durable publication, idempotent overlap, checkpoint/session/receipt persistence and safe retention implement existing backend obligations. Reuse state::FileBackend rather than duplicate state recovery. Typed commit framing is physical persistence, not a cross-type transaction abstraction. Controlled fault backends remain for injected failures; real persistence stories and benchmarks use shipped JsonStorage.

No old baseline was captured. The new shipped backend establishes the first baseline; it must not be compared as an identical harness against the former test adapter. Subsequent shared performance changes use identical JsonStorage workloads. Start measurements after compilation finishes and the user signals the machine is ready; otherwise wait for sustained low CPU and disk activity. Independent benchmark categories may run concurrently for ballpark measurements, as authorized by the user; disclose contention and retain the same concurrency arrangement for comparisons. Initial user approval to fix the reference fixture is superseded by shipping and using JsonStorage.

## Reference Configuration

These values are explicit benchmark consumer policy, not library Default implementations. Use the same settings and workloads before and after improvements.

| Setting | Value |
| --- | --- |
| Pending write bytes | 64 MiB |
| Pending write batches | 256 |
| Active / pending queries | 4 / 64 |
| Response rows / bytes | 4096 / 1 MiB |
| File transfer chunk | 1 MiB |
| Filesystem polling / entries / event capacity | 100 ms / 100,000 / 1024 |
| Record / sample batch | At most 4096 items and 1 MiB |
| Independent file operation limit | 4 after introducing the explicit setting; old worker baseline remains serial |

Input generators are bounded and deterministic. Records carry a sequence and 256-byte payload; samples carry a sequence/timestamp and f64 value. One million samples represent one second at 1 MHz; the throughput benchmark does not sleep to pace production. Persistent storage uses shipped JsonStorage and native file storage, whose performance is representative rather than a worst-case guarantee.

## Workloads And Measurement

Storage measures 30 MiB file publication/download, 10,000 records and 1,000,000 samples with symmetric writes/reads. Writes include durable completion; reads start from durable data and time full bounded consumption, excluding setup writes. Correctness and cleanup remain outside timing where feasible without retaining entire datasets. Include concurrent distinct files, ingestion admission versus durable completion, and relevant query/recovery coverage. Keep total generated and retained memory bounded.

Sidecar and guest each measure 10,000 records, a 30 MiB file, and 1,000,000 samples transferred host-to-plugin and plugin-to-host through real process/component boundaries. Use bounded typed batches and wait for receiver completion; verify ordering, counts, and content outside timing. Prepare generated data outside timing, and avoid measuring an echo as two independent directions. Startup and command latency are supplementary, not substitutes for these six transfer cases per carrier. Shared Schematic transfer contracts and handlers are enabled by a benchmark feature on the shipped example executables; ordinary product catalogs remain unchanged. Both directions compare unary calls carrying bounded batches against a stream carrying the same batch sizes and total dataset. Run the three datasets concurrently on one reused host/plugin per carrier, isolated by transfer ID; run sidecar and guest suites concurrently. Modes and directions remain sequential within a dataset. External gRPC measures a real client/server transport boundary. Distinguish startup from steady-state operation; warmup and sampling are identical before/after. Reuse shipped Schematic fixtures where applicable and canonical bench-built artifacts. Do not infer network or process costs from in-process codec benchmarks.

Native Criterion results and runner peak-memory evidence are authoritative artifacts. Preserve baseline separately from the post-change run; do not promote a failing baseline. Under 30 seconds per large storage workload is an initial evaluation target, not a correctness-test timer or an unmeasured guarantee. Millisecond commands and second-scale large files remain optimization goals subject to measured platform evidence.

## Storage Ownership And Scheduling

Move existing RecordInput, StreamInput, ObjectValue, file Vec and recovery payloads into admission; retain original allocations through Arc<Vec<u8>> or existing ownership-bearing types rather than Vec-to-Arc-slice relocation. Retained pending work survives backend attempts. Shared recovery spans replace deep-copy batching and blocking-adapter payload clones. Preserve shared Arrow allocation ownership through slices and IPC views.

One supervised worker dispatches bounded independent file Create/Resume/Write/Abort work; each file retains one logical command and in-flight attempt. Cursor-bearing persistence and file Finish remain ordered. Shutdown settles dispatched work before backend teardown. Preserve current conservative backend-wide failure admission gating initially; always settle already-dispatched work. Permanent failures remain visible and retry policy is not multiplied across layers.

## Transport Boundaries

Keep existing bounded operation channels and sequential gRPC forwarding. Sidecar request/reply envelope sizes are counted over borrowed values without allocating an encoded envelope; child-call admission includes the actual portable context. Distinguish logical frame size from the physical four-byte length prefix. Remove encoding-for-sizing and redundant Postcard/Protobuf buffers where compatible; preserve validation. Sidecar framing should retain incoming backing storage instead of copying payload extraction and avoid duplicating outgoing payload solely to frame it. Scalar-only writers coalesce frames up to 64 KiB into the existing reusable writer scratch; larger frames retain the split payload path. Wire credits cover encoded frames, while each driver separately owns one scratch buffer bounded by the protocol frame ceiling. Guest work is evidence-driven and preserves component isolation. No shared-memory IPC redesign is assumed. Supplementary gRPC codec cases measure Postcard encoding/decoding and Protobuf encoding/prepared decoding at the same payload sizes; these newly added stages have no historical baseline.

## Ownership And Migration Inventory

Storage owner: harmos-domain/model/storage; harmos-storage admission, objects, staging, recovery, Arrow, query, batching, worker and config; runtime/storage callers; associated storage tests and docs examples. Transport owner: harmos-domain/message accounting; harmos-sdk/grpc; harmos-sidecar transport and callers; runtime grpc/guest callers and focused tests. Benchmark owner: runtime benches, relevant SDK/sidecar benches, Cargo targets, canonical fixture/task selectors and README benchmark guidance. Attachment owner: runtime/storage/consumer.rs and attachments.rs, runtime stream batch selection, their consumers/tests, and examples/schematic/storage journal/stream adapters. Preserve shared row fan-out; inspection may borrow. Recovery redelivery shares exact encoded bytes rather than reconstructing canonical JSON. Orchestrator owns specification, manifests/task coordination and final validation. Writers receive disjoint assignments before editing.

## Execution Order

1. Implement independent typed backend writes and shipped JsonStorage, finish benchmark configuration/fixtures/category migration, and establish the first shipped-backend baseline.
2. Implement remaining owned storage/accounting, transport improvements and bounded file scheduling; migrate every affected consumer and focused test. Compare only runs using the same backend and harness.
3. Close source inventory, repeat identical benchmarks, compare native results and repair regressions.
4. Present the candidate for delivery authorization; required full native CI proof follows the authorized push workflow.

The user's explicit baseline/post benchmark request authorizes benchmark builds and runs during implementation. No ordinary full validation lane runs before source closure. No unrelated OpenSpec change is activated.

## Unified Filesystem Contract

The user expanded this same delivery to ReadFile, HashFile, FollowFile, CaptureFile, ListDirectory and WatchDirectory, plus applicable benchmarks. These are shared operations usable from gRPC, native plugins and Wasm guests, without extra per-plugin allowlists or carrier exclusions. Local paths resolve anywhere accessible to the executing host process; no configured-root boundaries or path allowlists apply. The user approved no symlink traversal, case-sensitive slash-relative globs and capture rejection on detected source mutation. Local file/directory operations have no path-scoped grant; ordinary operation exposure still applies. Existing stored-file scopes remain enforced. Host permission errors remain explicit.

FileSource is Stored(FileReference) or Local(PathBuf). ReadFile consumes source/range and streams bounded shared batches; HashFile returns bytes_read and SHA-256. FollowFile uses Beginning, End or At(offset) and emits bounded data plus explicit truncation/replacement events. Stored files remain immutable; local reads/hashes may observe mutation. CaptureFile creates an independent immutable durable file, hashes while transferring, and distinguishes bounded copying from stable-source checks; neither promises an atomic snapshot during concurrent mutation. Existing staged-production following remains a distinct necessary capability, not a compatibility alias.

ListDirectory and WatchDirectory share DirectorySelection { root, recursive, rules }. Ordered include/exclude rules use slash-relative paths: * stays within a segment, ** crosses segments, last match wins, unmatched excluded. Excluded directories are still traversed when descendants can match. Watch events are Existing, InitialScanComplete, Created, Modified, Removed, Renamed and RescanRequired; initial enumeration is coordinated with changes and overflow never silently loses data. Discovery/accounting belongs in Harmos; artifact classification and completion decisions remain consumer responsibilities.

Unique ReadBatch projection consumes rows through try_map or try_map_with, rejects aliased inputs before conversion and returns an ordinary shared owner for the result. Nested retained pages keep their own owners. Borrowed inspection remains preferred when no owned conversion is needed; consumers bound any new output and scratch allocations.

Composed typed/Arrow queries, object/file reads and retained collections use StorageManager and ordinary shared batch ownership. Consumer settings bound active/pending queries, pending write bytes/batches and serialized response pages; consumers bound their own retained collections and conversion scratch. The superseded ReadSession, RetainedMemory and shared read pool are removed. Explicit cursors and object versions retain their meaning. Readers wait within configured query admission and release their active slot on cancellation; retained returned pages remain valid independently.

ArrowBatch consuming projection moves its RecordBatch into the output. FileReadBatch projection delegates to its unique ReadBatch owner, so retained aliases refuse an owned move. Arrow-to-IPC conversions retain shared backing while transporting the required representation; no storage allocation claims are transferred or duplicated.

Managed-journal recovery borrows raw JSON for transaction decoding and skips unknown record payload trees. Recognized Prepared payloads and consumer application state keep their existing ownership; raw read-buffer accounting does not claim to bound arbitrary consumer deserialization.

Consumer integration preserves distinct contracts: path-based CaptureFile does not accept an already-open anonymous source or resume a caller-owned producer across failures; consumers retain their existing freeze/retry and accepted-prefix checks. Backend row-at-a-time encoding reduces retention; backend workers own their actual buffers and consumers bound conversion scratch. Consumer Value depth/cell bounds remain until a separately specified admitted conversion and destruction contract replaces them. These safeguards are not removed by the new filesystem operations.

Reuse ordinary shared batch ownership for native copy/hash/read buffers and typed pages. Compose filesystem, capture/replay/state settings with their current resource owners; transports own actual framed payload fit and admission. New local/stored read/hash/capture/follow workloads use 30 MiB files; directory list/initial-watch uses 1,000 entries and watch-create measures one native creation through notification. New workloads have no historical baseline and will be labeled accordingly. All implementation lands together; take one final measurement after source closure rather than intermediate benchmark rounds.

Storage-authoritative observers use explicit saved cursors. wait_saved_after(after) passively returns a strictly newer saved cursor using existing Inner.changed subscription before state inspection; updates may coalesce. Available durable progress is returned first; otherwise close/not-ready/persistence failures terminate the wait. Cancellation drops only its waiter. It validates store history and never requests persistence, clears failures or triggers retries. Existing FollowStream live tails remain accepted data and are not a durable change signal; no sample cache or scheduler is added.

Arrow payload encoding uses the shared counting/encoding path and bounded traversal scratch. Consumers retain converted rows through their own output owners and bound their own decoding and collection scratch; no storage read-pool admission or resident-size multiplier is introduced.

New operations and helpers follow the existing observability contract: named storage/carrier spans distinguish admission, queueing, execution and backend work; captured request dispatch/context survives spawned or blocking work and cancellation. Instrumentation skips payload contents and raw user paths. Reuse existing context and outcome machinery; synchronous hot loops do not create per-element spans or format data when tracing is disabled.
