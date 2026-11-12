# Operation Grammar Design

Status: locked by the owner on 2026-11-12. Nothing here claims implementation.
Only the explicitly listed open questions await decisions when their work is reached.

## Context

See proposal.md for motivation and decision status. Both supplied research
reports were read in full. Current declarations, provider traits, protocol rules,
SDKs and documentation were checked where locally available. Evidence paths below
are relative to this repository; sibling facts are described generically.

## Goals and Non-Goals

Make public operations predictable without flattening storage guarantees. Keep
consumer policy with consumers, and retain native storage optimization and
recovery responsibilities. Do not add a second scheduler, silently migrate stored
encodings or promise arbitrary historical presentation. Keep the library protocol,
SDKs and conformance suite self-sufficient without product vocabulary.

## Shared Grammar and Vocabulary

The following shared text uses the supplied decisions and both reports' Shared
Vocabulary sections. Product-specific meanings belong in the next section.

- **Names (Owner):** symmetric, predictable `VerbResource`; singular for one
  resource and plural for collections. Requests are `<Method>Request`, replies
  are `<Method>Response`. **Owner:** always the response wrapper, never a bare
  resource. Operation IDs are namespaced and derive mechanically from the method;
  their namespace includes the service group, without a second authored alias.
- **Ordinary verbs (Owner):** the closed set is Get, List, Watch, Set, Create,
  Delete and Append. Get observes one identified or derived value; List enumerates
  one bounded selection; Watch opens that selection and follows changes; Set
  establishes writable value fields; Create admits a lifecycle resource; Delete
  removes or releases it; Append adds ordered immutable data such as records,
  samples, log lines and upload bytes. Update, Read, Browse, Put, Edit, Add, Remove, Follow, Scan,
  Inspect and Register are not verbs in this grammar.
- **Set (Owner):** Get returns the complete writable value and its revision; Set
  sends that whole value and the expected revision from Get. A stale revision
  refuses atomically with a conflict naming the action: get again and retry.
  Field masks were rejected because path validation, nested-path rules, per-language helpers and subtle errors add complexity to small writable resources while a revision guard already prevents lost updates.
  Read-only and runtime-owned facts do not acquire a Set merely for symmetry.
  Split an unreasonably large writable part into separately settable sub-resources,
  each with its own Get and Set. Product execution rename and archive merge into
  SetExecution carrying the whole small writable part returned by GetExecution;
  only that identified execution changes.
- **Domain verbs (Owner):** use a domain verb only where a standard verb would
  hide a side effect, and state its one meaning. Start activates; Stop finishes at
  a safe boundary; Cancel interrupts; Restart replaces an activation; Seal closes
  an evidence lifecycle; Publish commits prepared visibility; Import admits an
  external representation; Export produces an external representation; Invoke
  calls a declared operation; Advance materializes a projection frontier; Capture
  freezes external data. PublishFileProduction validates and publishes an upload;
  DeleteFileProduction abandons its unpublished session. Plugin lifecycle verbs keep
  their distinct meanings. The clock wait keeps the name Sleep.
- **Operation and invocation (Owner):** an operation is a declaration, an
  invocation is one execution of it, and an attempt is one retry attempt.
- **Identifiers and scope (Owner):** opaque resource and `*_id` strings;
  `name` identifies a full resource, `parent` its ownership. Invocation and child
  IDs are correlation identities, never authorization or deduplication tokens.
  Scope is optional opaque caller-bound context, inherited unchanged; neither a
  grant nor a payload field from which authority is inferred. Requiring it is an
  operation's decision.
- **Position (Owner):** `(history, sequence)` within one ordered history.
  History includes store identity and epoch. Never compare positions from different
  histories. A position is not wall time, a process epoch or a page token.
- **Cut (Owner):** an atomic read selection, potentially containing several
  positions/revisions. A cut is not a timestamp, page token, process epoch, snapshot
  lease or seal digest. A scalar sequence alone cannot represent a composite cut.
- **Reads (Owner):** one read path per kind of data; optional position `at`;
  absent resolves to latest; the reply states `resolved_at`; paging and watches
  follow the same rule. Accept `at` only as a position a previous reply returned
  for consistency, or a product-defined terminal boundary. Arbitrary earlier
  positions are not offered. The product decides what latest and its terminal
  boundary mean; the library does not define product lifecycles.
- **Latest and durability (Owner):** latest resolves once, at read/watch
  opening, to the latest durable position without waiting. It does not wait for
  accepted data that is not yet saved. A caller that needs its own write visible
  passes the durable position from that write's receipt as `at`. The newest durable
  position must trail the newest accepted data by no more than a stated p95 bound,
  independent of payload size: 250 ms at the 95th percentile.
  Prove it with the Nightly durable-lag benchmark below. Try smaller flush pieces
  and size-triggered flushing first if it is missed; resolving reads against a
  snapshot of accepted data is the named fallback, not built.
- **SDK ownership (Owner):** language SDKs stay in the library because other
  products will also host plugins on it; product SDKs keep building on them.
  Protocol, SDKs and conformance remain self-sufficient without product vocabulary.
- **Paging (Owner):** `page_size`, `max_bytes`, `page_token`, `next_page_token`,
  `resolved_at`; tokens bind selection, ordering, cut and schema/projection revision.
  Stable ordering has an identifier tie-breaker and exclusive continuation. Bound
  rows and bytes; work bounds remain separate. Expired or mismatched tokens fail
  explicitly, never restart silently at latest.
- **Watching (Owner):** use optional `at` and return `resolved_at` under the read
  resolution rule. Exact opening, event and gap semantics remain open. The
  boundary/snapshot-then-changes union below is a candidate for that task; it does
  not settle the open question.
- **Time (Owner):** protobuf Timestamp for instants and Duration for elapsed
  budgets; host evidence time and optional source time remain distinct. Acceptance
  and saved time are separate. Source-time windows are half-open filters, separate
  from ingestion positions.
- **Failures (Owner):** one Failure vocabulary: classification, stable code,
  message, action, operation, plugin, cause and safe details; preserve causes and
  map consistently to canonical gRPC status/details. Diagnostics remain separate.
  Missing evidence, gaps and unavailable positions cannot become empty success.
- **Receipts (Owner):** admission, application and durability are separate
  facts. Receipts identify the semantic mutation and accepted/durable positions;
  accepted is not durable. Same key and same normalized request replay the original
  outcome; changed payload conflicts. Declare deduplication lifetime; expired keys
  fail rather than silently repeat irreversible work.
- **Retry policy (Owner):** one policy vocabulary for invocation budget,
  per-attempt budget, cancellation grace, total attempts, backoff and replay safety.
  Total attempts includes the first attempt. Backoff has one fixed/exponential
  representation with an explicit jitter choice. Retry only explicitly replay-safe
  operations; correlation IDs do not deduplicate. A per-call override changes only
  that call. Consumer-owned budgets and defaults remain with consumers.
- **Transfers (Owner):** resumable unary uploads and ranged unary downloads use
  one mechanism for clients and plugins. Create an upload session, Append ordered
  chunks at stated offsets, Get status to resume, Publish to expose content, and
  Delete to abandon. Downloads repeat Get with offset and maximum bytes, returning
  total length and digest. Configuration bounds chunk and range sizes.
- **Streaming (Owner):** never generate streaming twins automatically. Use
  unary calls plus Get and Watch on the resource. One managed streaming method
  replaces the ordinary gRPC method only where a cooperative final result after
  cancellation is actually promised. Native gRPC cancellation abandons observation
  and cannot promise that final response; accepted durable work survives lost
  observers. Server streaming is reserved for Watch; plugin protocol carriers
  retain their own streams. Get/List and transfers are bounded unary calls.

## Library Design

Choices in this section are Owner decisions, except the explicit open questions.
The shared vocabulary does not move application scheduling or metadata
ownership into the library.

### Library Interpretation of Shared Decisions

The shared vocabulary uses generic resources. The product decides what latest
means: a running execution uses its live frontier and a sealed execution uses its
seal. The shared glossary's terminal-boundary/digest distinction refers only to
that product latest selection; the library does not own those product states.

**Owner, 2026-11-12:** Cancel applies to running work only; queued product control
refuses Cancel with an error naming the action. Stop finishes at a safe boundary
and remains distinct. Renaming changes one identified resource, not its sibling
attempts; the library preserves the product-selected mutation scope.

**Owner:** domain lifecycle meanings are Materialize (produce a catalog-defined
resource), Initialize (prepare invocation context), Execute (perform a declared
step), Evaluate (determine a criterion result), Cleanup (release context resources),
Copy (copy selected file content), Prepare (reserve an external effect's identity
and preconditions) and Confirm (record that prepared effect's outcome). These are
application actions rather than new generic library RPCs.

**Owner:** Set validates and atomically replaces the whole writable value under
its expected revision. Saved-data watches must not label accepted data durable;
exact watch event and opening semantics remain an open question for their task.

### Current Evidence and Corrections

| Finding | Current Tree Evidence and Interpretation |
| --- | --- |
| 41 built-ins | `crates/harmos-operations/src/{host,lifecycle,storage,streams,clock}/`: 1 catalog, 3 invocation controls, 34 storage, 1 publication, 2 clock operations. Clock is registered separately from `builtins_catalog()`. |
| 30 projected bindings | `const GRPC` declarations in those directories: 24 storage, 4 host/control and 2 clock. |
| 4 plugin carriers | `proto/harmos/v1/plugin.proto` and runtime `plugin/grpc/{connection,invoke,publish}.rs`; all four SDKs implement them. |
| 11 static Storage methods | `proto/harmos/v1/storage.proto:service Storage` is a subset of 34 declarations. Rust `sdk/rust/src/storage_contracts.rs` also binds 11 requests. |
| Automatic twins | `crates/harmos-runtime/src/grpc/projection.rs` emits `Run{method}` and managed envelopes unconditionally. |
| Durable snapshots | `crates/harmos-storage/src/snapshot.rs` and `docs/storage.md` reject positions beyond saved progress; follows also join accepted data today. That is not an existing durable-watch contract. |
| Flush cadence and commits | `crates/harmos-storage/src/config.rs` defines 100 ms, a 1 MB batch target and a 16 MB cap; `worker.rs` schedules flushes; `sqlite/mod.rs` sets WAL and `synchronous=FULL`. Each batch commits synchronously under `backend.rs`; flush duration adds to the cadence and grows with bytes written. Nothing measures durable lag today. |
| Native inventory | `StorageBackend` in `backend.rs` has 58 hooks; `StateBackend` in `state/mod.rs` has 8. Other methods in that module are implementations, not extra provider hooks. |

The library report's present-operation totals agree with this tree. Its suggested
WaitTime is superseded by Sleep. Its bare-resource replies are superseded by response wrappers; whole-value
revision-guarded Set is locked. Its barrier-before-
latest recommendation and the product report's latest-accepted wording are
superseded by the owner's latest-durable selection. Its 34-gRPC method/204-message
replacement budgets do not describe this candidate: unary sessions need client
bindings and wrappers change reachable messages. Do not carry those totals forward.

Neither report supplied measured SDK sizes. The census below supplies current-tree
facts rather than inferring that SDKs are only generated stubs. Some existing main
specifications still describe retired macro authoring and “thirty built-ins”;
current source and the existing authoring-without-macros proposal distinguish that
history from landed inventory. This change adds wire requirements without
rewriting that unrelated migration.

### Durability Choice and Alternatives

**Owner, 2026-11-12:** latest resolves to the
latest durable position without waiting; a live view may lag newest accepted data
only if that lag stays small for large payloads as well as small ones. The newest
durable position must trail newest accepted data by no more than 250 ms
at p95, independent of payload size.

The current 100 ms flush cadence is not proof: each flush adds its own duration,
which grows with what is written. Batches target 1 MB and are capped at 16 MB,
each committed synchronously. Scheduling, backlog, separate typed writes and
persistence failures can increase lag, and nothing measures durable lag today.
Reads incur their normal query cost but no wait for newer accepted writes.
Persistence errors stay visible.
Consumers choose budgets; this proposal adds no library storage-capacity defaults.

Add a reusable backend benchmark to the Nightly benchmark set, following storage
benches' stable `integration/<case>` naming: `integration/durable_lag` with
`small_high_rate`, `multi_megabyte` and `mixed` load cases. Measure acceptance-to-
durability delay for admitted positions and report p50, p95 and maximum lag for
each load, including persistence backlog and synchronous commit contribution.
Consumers must be able to run the same case against their own backend. Assess
each load against the owner's bound rather than hiding large-payload lag in an
aggregate. This is a benchmark measurement, never a wall-clock test assertion.
Deterministic mechanism tests instead prove that flushes are byte-bounded and
large input is split so no single commit exceeds the configured size.

If the measured bound is missed, first try smaller flush pieces and flushing as
soon as a size threshold is reached, not only on the cadence, then remeasure all
three loads. If that is insufficient, the named fallback is reads resolving
against a snapshot of accepted data. That fallback is not built by this proposal.

| Alternative | Cost and Disposition |
| --- | --- |
| Latest durable without waiting | Owner: coherent saved data and no new persistence wait; requires payload-size-independent p95 lag within 250 ms, proven by the three-load Nightly benchmark. |
| Barrier on every latest read | Not chosen: latency depends on queued data and a synchronous commit; a passive read requests persistence. An explicit durability wait remains available to writers. |
| Snapshot of accepted data | Named fallback, not built: use if smaller flush pieces and size-triggered flushing still miss the measured bound. Requires a snapshot/retention mechanism spanning pending typed data and mutable state; existing saved snapshots cannot do it. |

A write receipt containing only `accepted_at` cannot be used as proof of
durability. Obtain `durable_at` from that write's completion/status before a read
requiring read-your-write visibility. If an issued cut has expired, fail explicitly;
never fall back to a newer cut. Validate issued-position provenance and retention
without treating knowledge of a sequence as authorization.

### Representative Public Shapes

These excerpts express the locked contract, not compilable replacement files or field-number
compatibility commitments. Domain payloads named below already have owners; the
complete descriptor inventory is the second implementation step. Optional message
fields use protobuf message presence. The four representative resources are rows, objects,
invocations and file productions. Watch event unions illustrate candidate details
for the still-open exact Watch semantics; the naming and read-resolution rules
are locked.

```proto
import "google/protobuf/duration.proto";
import "google/protobuf/timestamp.proto";

message History { string store = 1; string epoch = 2; }
message Position { History history = 1; uint64 sequence = 2; }
message ReadCut {
  repeated Position positions = 1;
  repeated ResourceRevision revisions = 2;
}
message PageRequest {
  uint32 page_size = 1;
  uint64 max_bytes = 2;
  string page_token = 3;
}
message Receipt {
  string request_id = 1;
  Position accepted_at = 2;
  Position durable_at = 3;
  bool completed = 4;
}
message RetryPolicy {
  uint64 total_attempts = 1; // Includes the first attempt.
  google.protobuf.Duration initial = 2;
  google.protobuf.Duration maximum = 3; // Presence selects exponential backoff.
  bool deterministic_jitter = 4;
}
// Every rpc M has MRequest and MResponse, including empty responses.
message GetTimeRequest {}
message GetTimeResponse { google.protobuf.Timestamp now = 1; }
message SleepRequest { google.protobuf.Duration duration = 1; }
message SleepResponse {}
```

```proto
message ListRowsRequest {
  QuerySource source = 1;
  ReadCut at = 2;
  repeated Transform transforms = 3;
  PageRequest page = 4;
}
message ListRowsResponse {
  repeated StoredRow rows = 1;
  ReadCut resolved_at = 2;
  string next_page_token = 3;
}
message WatchRowsRequest { ListRowsRequest selection = 1; }
message WatchRowsResponse {
  ReadCut resolved_at = 1;
  oneof event {
    RowsSnapshot snapshot = 2;
    RowsChange change = 3;
    ResyncRequired resync_required = 4;
    Position durable_progress = 5;
  }
}
message AppendRecordsRequest {
  string parent = 1;
  string request_id = 2;
  repeated Record records = 3;
}
message AppendRecordsResponse { Receipt receipt = 1; }
```

The rows union covers records, samples and object revisions with the same source
selector; no separate follow implementations survive. Filter progress and bounded
initial pages are candidate Watch details pending that task. Native
Arrow/index/projection mechanisms remain
distinct from application-level selection. Get/List return bounded unary pages
or ranges; callers repeat reads for further data. No streaming download remains.

Plugin WatchSamples observes publication demand; it has no historical List on the
plugin carrier. Retained sample reads belong to host storage. Do not manufacture
catalog watches without an actual change feed, and retain device-driven streams
alongside host-clock interval streams.

```proto
message GetObjectRequest { string name = 1; ReadCut at = 2; }
message GetObjectResponse {
  ObjectValue object = 1; // Complete writable value.
  bytes revision = 2;
  ReadCut resolved_at = 3;
}
message SetObjectRequest {
  ObjectValue object = 1; // Complete writable value from GetObject.
  bytes expected_revision = 2;
  string request_id = 3;
  bool create_if_absent = 4;
}
message SetObjectResponse {
  oneof outcome {
    AppliedObject applied = 1; // Value, revision and receipt.
    ObjectConflict conflict = 2;
  }
}
message GetInvocationRequest { string name = 1; ReadCut at = 2; }
message GetInvocationResponse { Invocation invocation = 1; ReadCut resolved_at = 2; }
message WatchInvocationRequest { string name = 1; ReadCut at = 2; }
message WatchInvocationResponse { Invocation invocation = 1; ReadCut resolved_at = 2; }
message CancelInvocationRequest { string name = 1; string reason = 2; }
message CancelInvocationResponse { Invocation invocation = 1; }
service Host {
  rpc GetInvocation(GetInvocationRequest) returns (GetInvocationResponse);
  rpc WatchInvocation(WatchInvocationRequest) returns (stream WatchInvocationResponse);
  rpc CancelInvocation(CancelInvocationRequest) returns (CancelInvocationResponse);
  rpc GetTime(GetTimeRequest) returns (GetTimeResponse);
  rpc Sleep(SleepRequest) returns (SleepResponse);
}
```

Whole-value object replacement must stay one backend atomic compare/exchange, not
read-then-write at the manager. Missing expected revision preserves explicit
create-if-absent semantics rather than silently allowing an unconditional overwrite.
A stale revision returns a conflict with action “get again and retry”.
GetObject also reports absence so create-if-absent remains explicit.
AppendObjectRevisions remains separate bulk immutable ingestion. The library's
invocation controls expose runtime-owned status, not writable status fields.

```proto
message CreateFileProductionRequest {
  FileMetadata metadata = 1;
  string request_id = 2;
}
message CreateFileProductionResponse { FileProduction production = 1; Receipt receipt = 2; }
message GetFileProductionRequest { string name = 1; ReadCut at = 2; }
message GetFileProductionResponse { FileProduction production = 1; ReadCut resolved_at = 2; }
message AppendFileBytesRequest {
  string name = 1;
  bytes writer_token = 2;
  uint64 offset = 3;
  bytes data = 4;
  string request_id = 5;
}
message AppendFileBytesResponse { FileProduction production = 1; Receipt receipt = 2; }
message CreateFileWriterRequest { string name = 1; bytes expected_writer = 2; }
message CreateFileWriterResponse { bytes writer_token = 1; FileProduction production = 2; }
message PublishFileProductionRequest {
  string name = 1;
  bytes writer_token = 2;
  uint64 length = 3;
  bytes sha256 = 4;
  string request_id = 5;
}
message PublishFileProductionResponse { FileReference file = 1; Receipt receipt = 2; }
message DeleteFileProductionRequest { string name = 1; string request_id = 2; }
message DeleteFileProductionResponse { FileProduction production = 1; Receipt receipt = 2; }
```

```proto
message GetFileBytesRequest {
  string name = 1;
  uint64 offset = 2;
  uint64 max_bytes = 3;
  ReadCut at = 4;
}
message GetFileBytesResponse {
  bytes data = 1;
  uint64 total_length = 2;
  bytes sha256 = 3;
  ReadCut resolved_at = 4;
}
```

Reuse this production as the upload session, with configured bounded unary chunks,
status, resume/fencing, integrity validation and terminal publication. Identical
bytes at an accepted offset replay successfully, even after reply loss; different
bytes at that offset conflict. Noncontiguous new offsets are refused. Delete of
an unpublished session abandons it without publishing; DeleteFile removes published
content under the still-open retained-reader contract. There is no implicit finish
or deletion on observation loss. Providers without sessions refuse explicitly.
Grants apply to creation, each chunk, status, publication and deletion.

Downloads repeatedly GetFileBytes with offset and maximum bytes; each unary reply
states the total content length and digest, letting clients resume and verify the
same content. Both client and plugin use this path; no streaming fallback remains.
Before deleting streaming transfers, measure large-file upload and download
throughput in the Nightly `integration/file_transfer` benchmark, reporting bytes,
chunk/range configuration, carrier and throughput. This is a benchmark, never a
test assertion or a new hard throughput threshold.

### Complete Public Operation Mapping

Every current logical operation appears once. Present types use the current
request and final reply; current stream item types are noted in the reason. Locked
request/reply names always follow the method-wrapper rule. All target names and
dispositions are Owner decisions, including retained names whose payloads change.

Evidence key: paths below are under `crates/harmos-runtime/tests/storage/` for
storage tests, under `crates/harmos-operations/src/` for declarations, and under
`crates/harmos-runtime/src/plugin/grpc/` for plugin-host callers. Named test sources
are invocation evidence, not claims that tests were run in this authoring task.

| Present Operation | Present Request → Final Reply | Locked Operation | Disposition and Reason / Caller Evidence |
| --- | --- | --- | --- |
| Plugin.Describe | DescribeRequest → Description | GetPlugin | Renamed; connection.rs and SDK servers use identity/catalog admission. |
| Plugin.Invoke | HostFrame → PluginFrame | InvokeOperation | Renamed; invoke.rs; duplex control carrier still conveys one unary operation attempt. |
| Plugin.Publish | PublishFrame → Samples | WatchSamples | Renamed; publish.rs; host-clock ticks and device publication remain distinct. |
| Plugin.Stop | StopRequest → StopResponse | StopPlugin | Renamed; connection.rs shutdown invokes cleanup. |
| host.catalog | Catalog → CatalogPage | ListCatalogEntries | Renamed; sdk/rust/src/client.rs calls bounded discovery. |
| host.operations.status | Status → InstanceState | GetInvocation | Renamed; tests/operations/host.rs invokes owned status. |
| host.operations.cancel | Cancel → InstanceState | CancelInvocation | Renamed; same host test; cancellation is an action. |
| host.operations.watch | Watch → InstanceState | WatchInvocation | Renamed; same host test; retain status events. |
| harmos.clock.now | Now → Time | GetTime | Renamed; four SDK contexts call the host clock. |
| harmos.clock.sleep | Sleep → SleepResponse | Sleep | Kept; four SDK contexts; locked clock action, not WaitTime. |
| host.publish | Publish → Published | AppendSamples | Renamed; streams/publish.rs and sdk/rust/src/wasm/context.rs publish immutable samples. |
| storage.status | StorageStatus → StorageInfo | GetStorageInfo | Renamed; generated_status.rs. |
| storage.capabilities | Capabilities → StorageCapabilities | GetStorageCapabilities | Renamed; generated_native_journey.rs. |
| storage.open_snapshot | OpenSnapshot → SavedSnapshot | CreateSnapshot | Renamed; same journey; retained backend read lease, not a second read family. |
| storage.release_snapshot | ReleaseSnapshot → Empty | DeleteSnapshot | Renamed; same journey; release the lease. |
| storage.scan_arrow | ScanArrow → ArrowPage | ListColumnRows | Renamed; same journey; preserve bounded physical work. |
| storage.scan_index | ScanIndex → IndexPage | ListIndexEntries | Renamed; same journey; ordered seek semantics remain. |
| storage.register_index | RegisterIndex → DefinitionRevision | SetIndexDefinition | Renamed; same journey; whole definition and expected revision from GetIndexDefinition; rebuild preconditions remain. |
| storage.register_projection | RegisterProjection → DefinitionRevision | SetProjectionDefinition | Renamed; same journey; whole definition and expected revision from GetProjectionDefinition. |
| storage.advance_projection | AdvanceProjection → DefinitionRevision | AdvanceProjection | Kept; same journey; side effect is checked materialization. |
| storage.read_projection | ReadProjection → ProjectionPage | ListProjectionRows | Renamed; same journey; exact revision/frontier availability. |
| storage.inspect_projection | InspectProjection → ProjectionState | GetProjectionStatus | Renamed; same journey; derived status is not writable. |
| storage.compare_exchange_object | CompareExchangeObject → CompareExchangeOutcome | SetObject | Renamed; same journey; whole writable object and expected revision from GetObject; atomic conditional outcome. |
| storage.query | Query → QueryCompletion | ListRows | Renamed; generated_query_batches.rs and generated_query_projection.rs; bounded unary pages replace QueryOutput streaming. |
| storage.follow_records | FollowRecords → StorageCursor | WatchRows, records selector | Merged; generated_record_follow.rs; shared boundary/change union. |
| storage.follow_stream | FollowStream → StorageCursor | WatchRows, samples selector | Merged; generated_stream_follow.rs; add consistent initial durable boundary. |
| storage.follow_objects | FollowObjects → StorageCursor | WatchRows, object-revision selector | Merged; generated_object_follow.rs; preserve revision data. |
| storage.write_schema | WriteSchema → StorageCursor | SetSchema | Renamed; operation_schemas.rs; whole small writable schema and expected revision from GetSchema; immutable revision rules remain. |
| storage.write_records | WriteRecords → WriteCompletion | AppendRecords | Renamed; generated_record_exchange.rs; bounded unary batch replaces DataBatch/WriteReceipt exchange. |
| storage.write_streams | WriteStreams → WriteCompletion | AppendSamples | Renamed; generated_exchange_cancellation.rs; preserve admitted outcomes with receipts. |
| storage.write_objects | WriteObjects → WriteCompletion | AppendObjectRevisions | Renamed; generated_object_exchange.rs; bulk ingestion does not replace atomic SetObject. |
| storage.write_file | WriteFile → FileReference | CreateFileProduction → AppendFileBytes → PublishFileProduction | Removed; generated_file_upload.rs; shared resumable unary session replaces client-stream FileBatch upload. |
| storage.create_file | CreateFile → FileProducer | CreateFileProduction | Renamed; generated_file_mutations.rs; create the upload/production session. |
| storage.append_file | AppendFile → FileProducer | AppendFileBytes | Renamed; same mutations; bounded immutable chunks. |
| storage.resume_file | ResumeFile → FileProducer | CreateFileWriter | Renamed; same mutations; fence the previous writer. |
| storage.finish_file | FinishFile → FileReference | PublishFileProduction | Renamed; same mutations; durable final reference. |
| storage.abandon_file | AbandonFile → FileProgress | DeleteFileProduction | Renamed; same mutations; Delete of the unpublished session abandons it. |
| storage.delete_file | DeleteFile → StagedFile | DeleteFile | Kept; same mutations; retained-reader decision remains open. |
| storage.read_file | ReadFile → FileCompletion | GetFileBytes | Renamed; generated_read.rs; streaming download removed in favor of bounded unary ranges with offset, maximum bytes, total length and digest. |
| storage.hash_file | HashFile → FileHash | GetFileHash | Renamed; examples/schematic/operations/src/transfer/hash.rs and guest SDK call it. |
| storage.capture_file | CaptureFile → FileReference | CaptureFile | Kept; generated_capture.rs; independent captured bytes. |
| storage.follow_file | FollowFile → FileCompletion | WatchFileBytes | Renamed; generated_capture.rs; FileEvent preserves replacement/truncation. |
| storage.follow_production | FollowProduction → FileProgress | WatchFileProduction | Renamed; generated_reconnect.rs and generated_future_offset.rs; writer fencing and retained chunks remain. |
| storage.list_directory | ListDirectory → DirectoryCompletion | ListDirectoryEntries | Renamed; generated_directory.rs; bounded DirectoryEntry selection. |
| storage.watch_directory | WatchDirectory → DirectoryCompletion | WatchDirectoryEntries | Renamed; same directory test; preserve DirectoryEvent and rescan-required semantics. |

Host.AppendSamples and Storage.AppendSamples have separate namespaces: the former
publishes a declared stream, the latter admits persisted sample batches. They are
not merged. Locked library namespaces include their group, for example
`harmos.storage.append_records`; the exact namespace transition and fingerprints
migrate together. Public names in the table do not rename arbitrary
consumer-defined operation IDs behind their owners' backs.

Invocation carrier suboperations remain: opening request/terminal result, unary
child request/result, parent cancel/child cancel, and publication opening/ticks.
They are frames, not additional public RPCs. Keep catalog sections, requirements,
visibility, fingerprints, digest, descriptors and failure causes used in admission.

### Added Get Mirrors and Product Mapping

| Added Library Operation | Exposure | Locked Value Contract |
| --- | --- | --- |
| GetIndexDefinition | Storage RPC | Whole writable index definition and revision, mirrored by SetIndexDefinition. |
| GetProjectionDefinition | Storage RPC | Whole writable projection definition and revision, mirrored by SetProjectionDefinition. |
| GetObject | Storage RPC | Whole writable object and revision, mirrored by SetObject; explicit absence for creation. |
| GetSchema | Native-only, matching SetSchema | Whole writable schema and revision, mirrored by SetSchema. |
| GetFileProduction | Storage RPC | Accepted/durable upload progress, writer generation and resume status. |

These small writable parts need no subdivision. Large product configuration or
artifact metadata must instead map to separately settable named sub-resources,
each with its own Get/Set and revision, rather than a monolithic replacement.
Product rename/archive setters map to GetExecution/SetExecution of the whole small
writable execution part; rename affects only the selected execution. These product
methods are not added to the library counts.

### Resulting Counts

| Count Boundary | Present | Locked Candidate |
| --- | ---: | ---: |
| Logical operations: plugin carriers plus built-ins | 45 = 4 + 41 | 47 = 4 + 43 |
| Dispositions of the 45 present rows | 45 | 4 kept, 37 renamed, 3 merged into 1, 1 removed; 42 surviving operations |
| Added operations | 0 | 5: four Get mirrors and GetFileProduction |
| Storage declarations | 34 | 36 = 34 − 2 watch merges − 1 streaming upload + 5 Get operations |
| Logical service groups when registered | 5 | 3: Plugin, Host, Storage |
| gRPC Methods at the stated exposure boundary | 64 = 4 + 30 ordinary + 30 managed | 42 = 4 plugin + 6 Host + 32 Storage; no automatic twins |
| Static Storage subset | 11 of 34 | 32 exposed declarations generated from the authoritative owner |
| Method request/response messages, all logical operations | No uniform wrapper rule | 94 = 2 × 47, including 84 for the 42 exposed gRPC methods and 10 for 5 native-only operations |
| StorageBackend hooks | 58 | 52 |
| StateBackend hooks | 8 | 8 |

The exposure calculation is 64 − 30 automatic twins − 1 WriteFile + 6 upload
session bindings + 3 exposed Get mirrors = 42. The six session bindings are
CreateFileProduction, GetFileProduction, AppendFileBytes, CreateFileWriter,
PublishFileProduction and DeleteFileProduction. The streaming ReadFile operation
is replaced in place by unary ranged GetFileBytes: zero extra gRPC methods. Get/List
streamed delivery is removed, not retained as an optional twin. Only Watch and
plugin carriers stream, subject to explicitly promised managed final-result gRPC methods
replacing ordinary gRPC methods one for one. Which methods promise those results remains
open, without changing gRPC method cardinality.

Message counts here count method-specific request/response definitions, qualified
by namespace (Host and Storage AppendSamples are distinct). They exclude domain
payloads, shared primitives, nested event/control frames and imported protobuf
messages; they are not a claimed transitive descriptor census. The complete proto
inventory task must enumerate those payload definitions and delete unreachable
messages as part of implementation, with no automatic managed-envelope additions.
The five native-only operations are Host.AppendSamples, GetStorageInfo, SetSchema,
GetSchema and DeleteFile. All six upload-session methods are exposed. Storage's
32 exposed plus four native-only declarations account for all 36 declarations.

### Complete Native Provider Mapping

These 66 present hooks are not additional RPCs. Locked names below are semantic
names; implementations retain normal language casing. Private provider hooks
such as explicit flush remain private mechanisms, not public grammar exceptions.
Every unchanged guarantee remains required even when a hook is renamed or merged.

| StorageBackend Hook | Locked Name | Disposition and Preserved Responsibility |
| --- | --- | --- |
| history | GetHistory | Renamed; store identity/epoch. |
| open | CreateStorageSession | Renamed; open durable provider. |
| cursor_saved | GetSavedPosition | Renamed; passive saved frontier. |
| close | DeleteStorageSession | Renamed; settle and release provider. |
| write_schemas | AppendSchemas | Renamed; assigned-position replay. |
| write_objects | AppendObjectRevisions | Renamed; immutable ingestion. |
| write_records | AppendRecords | Renamed; ordered immutable ingestion. |
| write_stream | AppendSamples | Renamed; ordered immutable ingestion. |
| write_files | AppendFileReferences | Renamed; immutable reference ingestion. |
| open_snapshot | CreateSnapshot | Renamed; exact saved backend token. |
| release_snapshot | DeleteSnapshot | Renamed; release retained token. |
| read_schemas | ListSchemas | Renamed; bounded saved selection. |
| read_objects | ListObjectRevisions | Renamed; bounded revision selection. |
| read_records | ListRecords | Renamed; bounded record selection. |
| read_stream | ListSamples | Renamed; bounded sample selection. |
| read_files | ListFiles | Renamed; bounded references. |
| read_identities | ListIdentities | Renamed; typed stored identities. |
| read_object | GetObject | Renamed; exact saved point read. |
| read_file | GetFileBytes | Renamed; bounded immutable range. |
| read_record_views | ListRecordViews | Renamed; optional shared immutable ownership. |
| seek_records | GetRecordPosition | Renamed; optional ordinal optimization. |
| work | GetWork | Renamed; instrumentation, not admission policy. |
| capabilities | GetStorageCapabilities | Renamed; incorporate support flags below. |
| scan_arrow | ListColumnRows | Renamed; response and physical work bounds remain separate. |
| scan_index | ListIndexEntries | Renamed; ordered seek and source authorization. |
| register_index | SetIndexDefinition | Renamed; rebuild/atomic revision preconditions. |
| index_definition | GetIndexDefinition | Renamed; definition and revision. |
| register_projection | SetProjectionDefinition | Renamed; revisioned definition. |
| projection_definition | GetProjectionDefinition | Merged with revision getter; absent revision selects current definition. |
| projection_definition_at | GetProjectionDefinition | Merged; explicit retained revision lookup remains for authorization. |
| projection_status | GetProjectionStatus | Merged with frontier getter; optional requested frontier. |
| projection_frontier | GetProjectionStatus | Merged; include readable durable frontier. |
| advance_projection_native | AdvanceProjection | Merged with checked-revision variant; optional expected revision. |
| advance_projection_native_at | AdvanceProjection | Merged; preserve atomic checked publication and fallback semantics. |
| publish_projection | SetProjectionOutput | Renamed; atomic durable publication, not another scheduler. |
| read_projection | ListProjectionRows | Renamed; exact frontier/revision selection. |
| retire_projection | DeleteProjectionHistory | Renamed; retained readers/revisions constrain reclamation. |
| compare_exchange | SetObject | Renamed; atomic conflict outcome and durable receipt. |
| retire_object_receipts | DeleteObjectReceipts | Renamed; explicit receipt retention. |
| release_object_history | DeleteObjectHistory | Renamed; release retained history. |
| prune_objects | DeleteExpiredObjectRevisions | Renamed; bounded collection under retention rules. |
| create_file | CreateFileStaging | Renamed; private staging remains available to capture. |
| write_file | AppendFileBytes | Renamed; per-file ordered staging I/O. |
| finish_file | PublishFileProduction | Renamed; integrity and publication position. |
| abort_file | DeleteFileStaging | Renamed; unpublished staging cleanup. |
| create_production | CreateFileProduction | Renamed; resumable production. |
| fence_production | SetFileWriterGeneration | Renamed; atomic expected/next fencing. |
| save_production | SetFileProductionCheckpoints | Renamed; internal restart metadata, not a read family. |
| recover_productions | ListFileProductionCheckpoints | Renamed; bounded recovery metadata. |
| file_progress | GetFileProduction | Renamed; explicit saved/accepted progress. |
| read_production | GetFileProductionBytes | Renamed; retained production range. |
| abandon_file | DeleteFileProduction | Renamed; terminate the unpublished session, preserving declared partial-data retention without publication. |
| delete_file | DeleteFile | Renamed to language form; retained-reader behavior remains open. |
| supports_file_sessions | GetStorageCapabilities | Merged; no separate boolean owner. |
| supports_file_deletion | GetStorageCapabilities | Merged; explicit deletion support. |
| supports_mutation_receipts | GetStorageCapabilities | Merged; explicit durable replay support. |
| save_mutations | AppendMutationReceipts | Renamed; persist original mutation outcomes. |
| recover_mutations | ListMutationReceipts | Renamed; bounded replay recovery. |

The 58-to-52 reduction is three getter/advancement pairs consolidated (minus 3)
and three support flags folded into capabilities (minus 3). No provider capability
is removed. Exact provider snapshot/revision hooks remain necessary internally;
the owner restriction on public `at` does not make native authorization/recovery
lookups arbitrary historical public APIs.

| StateBackend Hook | Locked Name | Disposition and Preserved Responsibility |
| --- | --- | --- |
| history | GetHistory | Renamed; recovery history identity. |
| open | CreateRecoverySession | Renamed; recovery position. |
| read_snapshot | GetRecoverySnapshot | Renamed; bounded durable recovery image. |
| write_snapshot | SetRecoverySnapshot | Renamed; durable recovery image replacement. |
| read_entries | ListRecoveryEntries | Renamed; bounded after/through journal range. |
| write_entries | AppendRecoveryEntries | Renamed; ordered journal ingestion. |
| flush | flush | Kept as private explicit durability barrier; not a latest-read side effect. |
| close | DeleteRecoverySession | Renamed; settle recovery owner. |

Manager conveniences retain one implementation behind typed reads/query,
follow/page, file producers/readers/captures, object groups, recovery readers and
explicit flush/flush-until/await-durable. Fold `_once` conveniences into the
mutation identity argument instead of maintaining a second implementation.
Local codec helpers are not RPCs; there is no current artifact/configuration RPC
family to remove. Finished file reads, local file watches and production watches
remain distinct because their replacement, fencing and terminal guarantees differ.

### SDK Ownership

**Owner, 2026-11-12:** other products besides the first will host plugins on this
library, so language SDKs stay here and product SDKs keep building on them; the
library protocol, SDKs and conformance suite remain self-sufficient, with no
product vocabulary. Moving SDKs into a product was considered and not chosen
because a second plugin-hosting product would have to reimplement the plugin
protocol in every language and the library could not be proven end to end by itself.
Current SDKs contain substantive protocol behavior, not merely protobuf bindings.

| Language | Library SDK Contains | Current Consumers |
| --- | --- | --- |
| Rust | Descriptor/fingerprint authoring, builders, invocation resources, server, clock/publication, guest/native support, generic managed application client and 11 storage request bindings. | Library examples, fixtures and runtime tests; product Rust SDK and its clients/plugins. |
| Python | Plugin decorators/descriptors, fingerprint validation, server/context, resources, clock/publication and generated protobuf/Storage stubs. | Python fixtures and SDK tests; product Python SDK and SDK stories. |
| C# | Plugin declarations, fingerprinting, service/context, child calls, clock/publication and generated build-time protocol clients. | C# fixture and SDK stories; product C# SDK consumes the pinned source. |
| C++ | Plugin builders, schema/fingerprints, gRPC server/context, resources, clock/publication and generated build-time protocol stubs. | C++ fixture/tests; product C++ SDK links the library SDK. |

Measured on 2026-11-12 from tracked source files only (`.rs`, `.py`, `.pyi`, `.cs`,
`.cpp`, `.hpp`, `.h`); counts include tests, examples and source tooling, exclude
locks/configuration and untracked build output. These are physical lines and file
bytes, not package download size. The product checkout was inspected read-only.

| Language | Library Files / Lines / Bytes | Product Files / Lines / Bytes |
| --- | --- | --- |
| Rust | 72 / 10,643 / 349,939 | 34 / 5,188 / 161,409 |
| Python | 25 / 4,590 / 216,097 | 22 / 3,035 / 104,027 |
| C# | 10 / 1,113 / 44,042 | 23 / 1,441 / 67,539 |
| C++ | 12 / 1,535 / 54,670 | 19 / 1,893 / 63,260 |

Excluding test/example directories leaves library Rust 7,534 lines, Python 4,138,
C# 1,113 and C++ 1,037. Python includes 3,131 tracked generated lines under `v1/`;
the other language protocol outputs are generated at build time or owned outside
the SDK directory. Rust's dependency includes `harmos-protocol` and derive support;
directory totals do not measure their transitive maintenance cost.

`proto/harmos/v1/behaviour.md` and `tools/conformance/` require a language-neutral
reference host and real plugin peers: descriptor normalization/fingerprints,
catalog admission, unary invoke/child calls, failure preservation, cancellation,
per-call policy isolation, host-clock ticks, device samples, demand closure,
reconnect/restart and managed/remote stop behavior. These remain library-owned
proof obligations. The library currently has only one generic managed application
client, in Rust. Only plugin semantic parity across Rust, Python, C# and C++, and
whether the generic managed client exists in all four or in Rust only, remain
open under SDKs; four-language client parity is not a landed fact or an approved
new requirement.

### Defects to Fix Regardless

| Defect | Evidence | Locked Independent Correction |
| --- | --- | --- |
| Python defaults to Public; other SDKs default to Private | `sdk/python/src/harmos/contracts.py`, Rust operation declarations, C# Operation and C++ header. | Align Python to Private and require explicit Public intent. |
| C# lacks invocation-local typed resources and returned cancellation reason | `sdk/csharp/Harmos/Context.cs`: internal cancelReason exists, CancelledAsync returns Task only; no public resource store. | Add per-invocation resource ownership and reason-bearing cancellation, with child settlement parity. These are not persisted settings. |
| Static storage service exposes a subset | 11 proto RPCs versus 34 authoritative storage declarations and 11 Rust request bindings. | Generate the exposed service/client contracts from one declaration owner; preserve grants and native-only boundaries. |
| Retry policy has two shapes | Plugin attempts/fixed backoff versus host max_retries/fixed-or-exponential/jitter; plugin declaration lacks replay safety. | One policy and replay classification, with call-local overrides. |
| Plugin proto says draft/unimplemented | First line of `proto/harmos/v1/plugin.proto` conflicts with four implemented SDKs and host runtime. | Replace the stale header with the current contract status during implementation. |

Scope accessors, inherited scope, client retained-byte admission and descriptor-only
message registration already exist here. Do not reimplement them based on stale
sibling pins. Typed operation discovery already fills CatalogPage.operations;
stored schema/identity catalogs remain JSON bytes and need a typed replacement.

The product reports additionally identify missing host operations still declared
by plugin SDKs and an older desktop generated client. Those repairs belong to the
product; its desktop move to the new host and grammar regeneration should be one
migration after the exact library revision is available.

## Risks and Trade-offs

- Receipt/position renames change fingerprints and admission: migrate descriptors,
  requirements and callers as one revision; never add silent compatibility aliases.
- Latest durable deliberately omits newer accepted data, and synchronous flush
  duration grows with payload size on top of the 100 ms cadence. The owner accepts
  this only within the stated payload-size-independent p95 bound (250 ms), not yet
  measured. Run the three-load Nightly benchmark;
  try smaller flush pieces and size-triggered flushing first if it misses the bound,
  then consider the named accepted-data snapshot fallback, not built here. Expose
  durability explicitly and retain writer barriers.
- Current follows can emit accepted data: changing them requires explicit durable
  boundaries and deterministic gap/reconnect proof, not a name-only edit.
- Unary sessions add round trips and remove the one-shot path on providers lacking
  sessions: document capability refusal and measure chunk overhead without changing
  consumer-owned concurrency/size budgets.
- Retained cuts and deletion can conflict: owner review must settle reader lifetime
  before removing or reclaiming content. No stored-data deletion is authorized here.

## Open Questions

Only these questions remain open. Settle each when implementation reaches its
owner; they do not block the other locked work. Each has a corresponding task.

1. **Four-Language Plugin Parity:** settle the semantic parity promise for Rust,
   Python, C# and C++.
2. **Generic Managed Clients:** settle whether generic managed-client coverage is
   Rust-only or includes the other languages.
3. **Issued Positions and Restart:** settle returned-position/page-token lifetime,
   expiry, provenance and retained composite selections across restart. Arbitrary
   earlier positions remain refused; product presentation retention stays there.
4. **Exact Watch Semantics:** settle opening snapshot/boundary, event/progress and
   gap/resynchronization rules, including external file/directory and runtime
   sources. Watch shapes earlier in this document are candidate details
   for that task, not an assertion that these exact semantics are settled.
5. **Deletion With Retained Readers:** settle reader lifetime and reclamation on
   supported backends before implementing deletion changes.
6. **Cooperative Final Results:** enumerate which operations actually promise a
   final result after cancellation; only those get a managed replacement gRPC method.

## Migration Plan

First implement shared primitives and both Nightly benchmark cases, then the
complete proto inventory without automatic twins. Change handlers, SDKs and every
affected library consumer together in compiling
slices, preserving existing data encodings. Reuse the existing production owner
for unary transfers and remove superseded gRPC methods/envelopes only after consumers
migrate and transfer throughput has been measured. Update conformance fixtures
and affected guides with each slice.

Publish/pin one exact library revision only under later authorization; the product
then adopts it, repairs its host gaps, and migrates SDKs, CLI, then desktop clients
together with removal of superseded hosts. Rollback uses a matched prior host/SDK
revision; it is not an adapter layer or an implicit storage reset. Storage format
changes require a separate data-migration decision. Final validation is separate
from source closure and follows focused proof on both required native platforms.

## Storage Implementation Ownership

`feat/gp-grammar-storage` owns `proto/harmos/v1/storage.proto`, `crates/harmos-storage/**`,
`crates/harmos-operations/src/storage/**`, storage handlers/tests/benches in runtime,
storage benchmark tooling and Nightly wiring, `docs/storage.md`, and the storage tasks/design
in this change. Shared protocol primitives and SDK bindings belong to the parallel core worker.
Streaming transfers remain until task 6.1; no accepted-data snapshot is implemented.

### Windows Durable-Lag Measurements

Measured on 2026-11-12 in the bench profile, SQLite WAL with `synchronous=FULL`.
Each load opens an empty store and a native worker. Small load offers 1,000 single
32-byte samples separated by 1 ms; large load offers 128 logical 4 MiB batches
(64 samples of 64 KiB); mixed load offers 1,000 batches with every sixteenth batch
large. Existing admission bounds split a logical batch when needed, without changing
its bytes. An independent observer measures admission-return to passive saved-frontier
notification; scheduler observation delay is included. Admission backpressure is separate.
These are local measurements, not assertions or native Linux/hosted proof.

The before run uses whole admitted provider batches; the after run splits at entry
boundaries around the existing 1 MiB target, retaining the existing 16 MiB admission
ceiling and size-triggered flushing. An indivisible row retains that ceiling. Durability
mode, cadence, workload, pending-write budget and deadlines are unchanged.

| Load | Before p50 / p95 / Maximum (ms) | After p50 / p95 / Maximum (ms) | Before / After Maximum Backlog (Bytes) | Before / After Commit Count | Before / After Synchronous Commit Total (ms) |
| --- | --- | --- | --- | --- | --- |
| `small_high_rate` | 53.96 / 99.11 / 104.45 | 53.10 / 98.69 / 103.67 | 18,032 / 18,032 | 24 / 23 | 19.00 / 17.70 |
| `multi_megabyte` | 141.18 / 149.83 / 157.95 | 175.54 / 229.27 / 258.32 | 12,602,304 / 12,602,304 | 128 / 1664 | 9459.83 / 11553.25 |
| `mixed` | 257.81 / 301.59 / 331.38 | 174.14 / 198.77 / 233.64 | 12,614,064 / 12,614,064 | 82 / 833 | 4778.64 / 5261.50 |

Mixed p95 missed 250 ms before splitting. All three after-run p95 values meet it;
the large-load maximum exceeds 250 ms, which is not a maximum-latency promise.
The increased commit count and total commit contribution remain visible. Accepted-data
snapshots were not built. The benchmark emits backlog positions and admission wait as
well as this table; Nightly retains the raw observations and platform-qualified metrics.
