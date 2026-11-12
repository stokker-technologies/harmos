# Operation Grammar Design

Status: proposed, not locked. Owner decisions dated 2026-11-12 are identified
below; every other design choice is proposed. Nothing here claims implementation.

## Context

See proposal.md for motivation and decision status. Both supplied research
reports were read in full. Current declarations, provider traits, protocol rules,
SDKs and documentation were checked where locally available. Evidence paths below
are relative to this repository; sibling facts are described generically.

## Goals and Non-Goals

Make public operations predictable without flattening storage guarantees. Keep
consumer policy with consumers, and retain native storage optimization and
recovery responsibilities. Do not add a second scheduler, silently migrate stored
encodings, promise arbitrary historical presentation, or decide SDK ownership.

## Shared Grammar and Vocabulary

The following shared text uses the supplied decisions and both reports' Shared
Vocabulary sections. Product-specific meanings belong in the next section.

- **Names (owner):** symmetric, predictable `VerbResource`; singular for one
  resource and plural for collections. Requests are `<Method>Request`, replies
  are `<Method>Response`. **Proposed:** always the response wrapper, never a bare
  resource. Operation IDs are namespaced and derive mechanically from the method;
  their exact namespace/group layout remains proposed, not a second authored alias.
- **Ordinary verbs (proposed):** the closed set is Get, List, Watch, Set, Create,
  Delete and Append. Get observes one identified or derived value; List enumerates
  one bounded selection; Watch opens that selection and follows changes; Set
  establishes writable value fields; Create admits a lifecycle resource; Delete
  removes or releases it; Append adds ordered immutable data such as records,
  samples and log lines. Update, Read, Browse, Put, Edit, Add, Remove, Follow, Scan,
  Inspect and Register are not verbs in this grammar.
- **Set (proposed):** Set takes an optional field mask; absent means the whole
  value; an expected revision guards it where the resource has revisions.
  Read-only and runtime-owned facts do not acquire a Set merely for symmetry.
- **Domain verbs (proposed):** use a domain verb only where a standard verb would
  hide a side effect, and state its one meaning. Start activates; Stop finishes at
  a safe boundary; Cancel interrupts; Restart replaces an activation; Seal closes
  an evidence lifecycle; Publish commits prepared visibility; Import admits an
  external representation; Export produces an external representation; Invoke
  calls a declared operation; Advance materializes a projection frontier; Capture
  freezes external data; Finish completes a production; Abandon terminates a
  production with different retained-data guarantees. Plugin lifecycle verbs keep
  their distinct meanings. The clock wait keeps the name Sleep.
- **Operation and invocation (proposed):** an operation is a declaration, an
  invocation is one execution of it, and an attempt is one retry attempt.
- **Identifiers and scope (proposed):** opaque resource and `*_id` strings;
  `name` identifies a full resource, `parent` its ownership. Invocation and child
  IDs are correlation identities, never authorization or deduplication tokens.
  Scope is optional opaque caller-bound context, inherited unchanged; neither a
  grant nor a payload field from which authority is inferred. Requiring it is an
  operation's decision.
- **Position (proposed):** `(history, sequence)` within one ordered history.
  History includes store identity and epoch. Never compare positions from different
  histories. A position is not wall time, a process epoch or a page token.
- **Cut (proposed):** an atomic read selection, potentially containing several
  positions/revisions. A cut is not a timestamp, page token, process epoch, snapshot
  lease or seal digest. A scalar sequence alone cannot represent a composite cut.
- **Reads (owner):** one read path per kind of data; optional position `at`;
  absent resolves to latest; the reply states `resolved_at`; paging and watches
  follow the same rule. Accept `at` only as a position a previous reply returned
  for consistency, or a product-defined terminal boundary. Arbitrary earlier
  positions are not offered. The product decides what latest and its terminal
  boundary mean; the library does not define product lifecycles.
- **Latest and durability (proposed):** latest resolves once, at read/watch
  opening, to the latest durable position without waiting. It does not wait for
  accepted data that is not yet saved. A caller that needs its own write visible
  passes the durable position from that write's receipt as `at`. No snapshot of
  accepted data is built. Measure visibility lag before lock.
- **Paging (proposed):** `page_size`, `max_bytes`, `page_token`, `next_page_token`,
  `resolved_at`; tokens bind selection, ordering, cut and schema/projection revision.
  Stable ordering has an identifier tie-breaker and exclusive continuation. Bound
  rows and bytes; work bounds remain separate. Expired or mismatched tokens fail
  explicitly, never restart silently at latest.
- **Watching (proposed):** opening returns a boundary/snapshot at the resolved
  cut, then changes strictly after it. Explicit `at` fixes the starting snapshot,
  not an eternal frozen subscription; gap detection and explicit resynchronization
  are mandatory. Filters can advance progress without emitting matching rows.
- **Time (proposed):** protobuf Timestamp for instants and Duration for elapsed
  budgets; host evidence time and optional source time remain distinct. Acceptance
  and saved time are separate. Source-time windows are half-open filters, separate
  from ingestion positions.
- **Failures (proposed):** one Failure vocabulary: classification, stable code,
  message, action, operation, plugin, cause and safe details; preserve causes and
  map consistently to canonical gRPC status/details. Diagnostics remain separate.
  Missing evidence, gaps and unavailable positions cannot become empty success.
- **Receipts (proposed):** admission, application and durability are separate
  facts. Receipts identify the semantic mutation and accepted/durable positions;
  accepted is not durable. Same key and same normalized request replay the original
  outcome; changed payload conflicts. Declare deduplication lifetime; expired keys
  fail rather than silently repeat irreversible work.
- **Retry policy (proposed):** one policy vocabulary for invocation budget,
  per-attempt budget, cancellation grace, total attempts, backoff and replay safety.
  Total attempts includes the first attempt. Backoff has one fixed/exponential
  representation with an explicit jitter choice. Retry only explicitly replay-safe
  operations; correlation IDs do not deduplicate. A per-call override changes only
  that call. Exact field names and defaults remain proposed until lock.
- **Uploads (proposed; recommendation accepted by owner):** resumable unary
  upload sessions replace streaming uploads; one mechanism serves clients and
  plugins because plugin operations are unary.
- **Streaming (proposed):** never generate streaming twins automatically. Use
  unary calls plus Get and Watch on the resource. One managed streaming method
  replaces the ordinary route only where a cooperative final result after
  cancellation is actually promised. Native gRPC cancellation abandons observation
  and cannot promise that final response; accepted durable work survives lost
  observers. Bounded output streaming does not require a managed twin.

## Library Design

All choices in this section are proposed unless explicitly marked as owner
decisions. The shared vocabulary does not move application scheduling or metadata
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

**Proposed:** domain lifecycle meanings are Materialize (produce a catalog-defined
resource), Initialize (prepare invocation context), Execute (perform a declared
step), Evaluate (determine a criterion result), Cleanup (release context resources),
Copy (copy selected file content), Prepare (reserve an external effect's identity
and preconditions) and Confirm (record that prepared effect's outcome). These are
application actions rather than new generic library RPCs.

**Proposed:** Set validates a complete candidate and atomically applies selected
writable fields. An explicitly empty mask is refused, not confused with absent
mask/whole-value replacement. Saved-data watches report subsequent durable progress
and never label pending accepted data durable. These elaborations require the
owner's lock with the shared recommendations.

### Current Evidence and Corrections

| Finding | Current Tree Evidence and Interpretation |
| --- | --- |
| 41 built-ins | `crates/harmos-operations/src/{host,lifecycle,storage,streams,clock}/`: 1 catalog, 3 invocation controls, 34 storage, 1 publication, 2 clock operations. Clock is registered separately from `builtins_catalog()`. |
| 30 projected bindings | `const GRPC` declarations in those directories: 24 storage, 4 host/control and 2 clock. |
| 4 plugin carriers | `proto/harmos/v1/plugin.proto` and runtime `plugin/grpc/{connection,invoke,publish}.rs`; all four SDKs implement them. |
| 11 static Storage methods | `proto/harmos/v1/storage.proto:service Storage` is a subset of 34 declarations. Rust `sdk/rust/src/storage_contracts.rs` also binds 11 requests. |
| Automatic twins | `crates/harmos-runtime/src/grpc/projection.rs` emits `Run{method}` and managed envelopes unconditionally. |
| Durable snapshots | `crates/harmos-storage/src/snapshot.rs` and `docs/storage.md` reject positions beyond saved progress; follows also join accepted data today. That is not an existing durable-watch contract. |
| Flush cadence and commits | `crates/harmos-storage/src/config.rs` defines 100 ms; `worker.rs` schedules flushes; `sqlite/mod.rs` sets WAL and `synchronous=FULL`. Backend writes complete durably under `backend.rs`. |
| Native inventory | `StorageBackend` in `backend.rs` has 58 hooks; `StateBackend` in `state/mod.rs` has 8. Other methods in that module are implementations, not extra provider hooks. |

The library report's present-operation totals agree with this tree. Its proposed
WaitTime is superseded by Sleep. Its bare-resource replies and full-value-only
Set are superseded by response wrappers and optional masks. Its barrier-before-
latest recommendation and the product report's latest-accepted wording are
superseded by the proposed latest-durable selection. Its 34-route/204-message
replacement budgets do not describe this candidate: unary sessions need client
bindings and wrappers change reachable messages. Do not carry those totals forward.

Neither report supplied measured SDK sizes. The census below supplies current-tree
facts rather than inferring that SDKs are only generated stubs. Some existing main
specifications still describe retired macro authoring and “thirty built-ins”;
current source and the existing authoring-without-macros proposal distinguish that
history from landed inventory. This change adds wire requirements without
rewriting that unrelated migration.

### Durability Choice and Alternatives

With the current 100 ms cadence and synchronous commits, a healthy live view is
expected to lag newest accepted data by about one interval plus a commit. This is
an estimate, not a measured upper bound: scheduling, backlog, separate typed
writes and persistence failures can increase lag. Reads incur their normal query
cost but no wait for newer accepted writes. Persistence errors stay visible.
Consumers choose budgets; this proposal adds no library storage-capacity defaults.

| Alternative | Cost and Disposition |
| --- | --- |
| Latest durable without waiting | Proposed: coherent saved data and no new persistence wait; accepts visibility lag. Measure lag and commit contribution before lock. |
| Barrier on every latest read | Not chosen: latency depends on queued data and a synchronous commit; a passive read requests persistence. An explicit durability wait remains available to writers. |
| Snapshot of accepted data | Not chosen: requires a new snapshot/retention mechanism spanning pending typed data and mutable state; existing saved snapshots cannot do it. |

A write receipt containing only `accepted_at` cannot be used as proof of
durability. Obtain `durable_at` from that write's completion/status before a read
requiring read-your-write visibility. If an issued cut has expired, fail explicitly;
never fall back to a newer cut. Validate issued-position provenance and retention
without treating knowledge of a sequence as authorization.

### Representative Public Shapes

These are proposed proto excerpts, not compilable replacement files or field-number
compatibility commitments. Domain payloads named below already have owners; final
descriptors must be locked before implementation. Optional message fields use
protobuf message presence. The four representative resources are rows, objects,
invocations and file productions.

```proto
import "google/protobuf/duration.proto";
import "google/protobuf/field_mask.proto";
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
initial pages remain explicit. Native Arrow/index/projection mechanisms remain
distinct from application-level selection. Get/List may deliver a bounded page or
byte range in frames when backpressure requires it; this is one declared route,
not a second automatically generated method.

Plugin WatchSamples observes publication demand; it has no historical List on the
plugin carrier. Retained sample reads belong to host storage. Do not manufacture
catalog watches without an actual change feed, and retain device-driven streams
alongside host-clock interval streams.

```proto
message SetObjectRequest {
  ObjectValue object = 1;
  google.protobuf.FieldMask update_mask = 2;
  bytes expected_revision = 3;
  string request_id = 4;
  bool create_if_absent = 5;
}
message SetObjectResponse {
  oneof outcome {
    AppliedObject applied = 1; // Value, revision and receipt.
    ObjectConflict conflict = 2;
  }
}
message GetInvocationRequest { string name = 1; }
message GetInvocationResponse { Invocation invocation = 1; }
message WatchInvocationRequest { string name = 1; }
message WatchInvocationResponse { Invocation invocation = 1; }
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

Masked object replacement must stay one backend atomic compare/exchange, not
read-then-write at the manager. Missing expected revision preserves explicit
create-if-absent semantics rather than silently allowing an unconditional overwrite.
AppendObjectRevisions remains separate bulk immutable ingestion. The library's
invocation controls expose runtime-owned status, not writable status fields.

```proto
message CreateFileProductionRequest {
  FileMetadata metadata = 1;
  string request_id = 2;
}
message CreateFileProductionResponse { FileProduction production = 1; Receipt receipt = 2; }
message GetFileProductionRequest { string name = 1; }
message GetFileProductionResponse { FileProduction production = 1; }
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
message FinishFileProductionRequest {
  string name = 1;
  bytes writer_token = 2;
  uint64 length = 3;
  bytes sha256 = 4;
  string request_id = 5;
}
message FinishFileProductionResponse { FileReference file = 1; Receipt receipt = 2; }
message AbandonFileProductionRequest { string name = 1; string request_id = 2; }
message AbandonFileProductionResponse { FileProduction production = 1; Receipt receipt = 2; }
```

Reuse this production as the upload session, with bounded unary chunks, status,
resume/fencing, integrity validation and terminal publication. Replaying the same
chunk identity/offset/data returns its receipt; changed data conflicts. Abandon
ends writing with existing partial-data retention; DeleteFile has distinct deletion
semantics. Capability refusal is explicit for providers without resumable sessions;
the old one-shot streaming fallback is removed, not silently selected. Grants
continue to apply to session creation, each chunk and finish.

### Complete Public Operation Mapping

Every current logical operation appears once. Present types use the current
request and final reply; stream item types are noted in the reason. Proposed
request/reply names always follow the method-wrapper rule. All target names and
dispositions are proposed, including retained names whose payloads change.

Evidence key: paths below are under `crates/harmos-runtime/tests/storage/` for
storage tests, under `crates/harmos-operations/src/` for declarations, and under
`crates/harmos-runtime/src/plugin/grpc/` for plugin-host callers. Named test sources
are invocation evidence, not claims that tests were run in this authoring task.

| Present Operation | Present Request → Final Reply | Proposed Operation | Disposition and Reason / Caller Evidence |
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
| harmos.clock.sleep | Sleep → SleepResponse | Sleep | Kept; four SDK contexts; proposed clock exception, not WaitTime. |
| host.publish | Publish → Published | AppendSamples | Renamed; streams/publish.rs and sdk/rust/src/wasm/context.rs publish immutable samples. |
| storage.status | StorageStatus → StorageInfo | GetStorageInfo | Renamed; generated_status.rs. |
| storage.capabilities | Capabilities → StorageCapabilities | GetStorageCapabilities | Renamed; generated_native_journey.rs. |
| storage.open_snapshot | OpenSnapshot → SavedSnapshot | CreateSnapshot | Renamed; same journey; retained backend read lease, not a second read family. |
| storage.release_snapshot | ReleaseSnapshot → Empty | DeleteSnapshot | Renamed; same journey; release the lease. |
| storage.scan_arrow | ScanArrow → ArrowPage | ListColumnRows | Renamed; same journey; preserve bounded physical work. |
| storage.scan_index | ScanIndex → IndexPage | ListIndexEntries | Renamed; same journey; ordered seek semantics remain. |
| storage.register_index | RegisterIndex → DefinitionRevision | SetIndexDefinition | Renamed; same journey; definition/rebuild preconditions remain. |
| storage.register_projection | RegisterProjection → DefinitionRevision | SetProjectionDefinition | Renamed; same journey; revisioned definition. |
| storage.advance_projection | AdvanceProjection → DefinitionRevision | AdvanceProjection | Kept; same journey; side effect is checked materialization. |
| storage.read_projection | ReadProjection → ProjectionPage | ListProjectionRows | Renamed; same journey; exact revision/frontier availability. |
| storage.inspect_projection | InspectProjection → ProjectionState | GetProjectionStatus | Renamed; same journey; derived status is not writable. |
| storage.compare_exchange_object | CompareExchangeObject → CompareExchangeOutcome | SetObject | Renamed; same journey; preserve atomic conditional outcome. |
| storage.query | Query → QueryCompletion | ListRows | Renamed; generated_query_batches.rs and generated_query_projection.rs; retain QueryOutput framing where bounded delivery needs it. |
| storage.follow_records | FollowRecords → StorageCursor | WatchRows, records selector | Merged; generated_record_follow.rs; shared boundary/change union. |
| storage.follow_stream | FollowStream → StorageCursor | WatchRows, samples selector | Merged; generated_stream_follow.rs; add consistent initial durable boundary. |
| storage.follow_objects | FollowObjects → StorageCursor | WatchRows, object-revision selector | Merged; generated_object_follow.rs; preserve revision data. |
| storage.write_schema | WriteSchema → StorageCursor | SetSchema | Renamed; operation_schemas.rs; identical immutable revision succeeds, conflict refuses. |
| storage.write_records | WriteRecords → WriteCompletion | AppendRecords | Renamed; generated_record_exchange.rs; bounded unary batch replaces DataBatch/WriteReceipt exchange. |
| storage.write_streams | WriteStreams → WriteCompletion | AppendSamples | Renamed; generated_exchange_cancellation.rs; preserve admitted outcomes with receipts. |
| storage.write_objects | WriteObjects → WriteCompletion | AppendObjectRevisions | Renamed; generated_object_exchange.rs; bulk ingestion does not replace atomic SetObject. |
| storage.write_file | WriteFile → FileReference | CreateFileProduction → AppendFileBytes → FinishFileProduction | Removed; generated_file_upload.rs; shared resumable unary session replaces client-stream FileBatch upload. |
| storage.create_file | CreateFile → FileProducer | CreateFileProduction | Renamed; generated_file_mutations.rs; create the upload/production session. |
| storage.append_file | AppendFile → FileProducer | AppendFileBytes | Renamed; same mutations; bounded immutable chunks. |
| storage.resume_file | ResumeFile → FileProducer | CreateFileWriter | Renamed; same mutations; fence the previous writer. |
| storage.finish_file | FinishFile → FileReference | FinishFileProduction | Renamed; same mutations; durable final reference. |
| storage.abandon_file | AbandonFile → FileProgress | AbandonFileProduction | Renamed; same mutations; retained prefix differs from deletion. |
| storage.delete_file | DeleteFile → StagedFile | DeleteFile | Kept; same mutations; retained-reader decision remains open. |
| storage.read_file | ReadFile → FileCompletion | GetFileBytes | Renamed; generated_read.rs; FileBatch bounded delivery remains available. |
| storage.hash_file | HashFile → FileHash | GetFileHash | Renamed; examples/schematic/operations/src/transfer/hash.rs and guest SDK call it. |
| storage.capture_file | CaptureFile → FileReference | CaptureFile | Kept; generated_capture.rs; independent captured bytes. |
| storage.follow_file | FollowFile → FileCompletion | WatchFileBytes | Renamed; generated_capture.rs; FileEvent preserves replacement/truncation. |
| storage.follow_production | FollowProduction → FileProgress | WatchFileProduction | Renamed; generated_reconnect.rs and generated_future_offset.rs; writer fencing and retained chunks remain. |
| storage.list_directory | ListDirectory → DirectoryCompletion | ListDirectoryEntries | Renamed; generated_directory.rs; bounded DirectoryEntry selection. |
| storage.watch_directory | WatchDirectory → DirectoryCompletion | WatchDirectoryEntries | Renamed; same directory test; preserve DirectoryEvent and rescan-required semantics. |

Host.AppendSamples and Storage.AppendSamples have separate namespaces: the former
publishes a declared stream, the latter admits persisted sample batches. They are
not merged. Proposed library namespaces include their group, for example
`harmos.storage.append_records`; the exact namespace transition and fingerprints
must be approved together. Public names in the table do not rename arbitrary
consumer-defined operation IDs behind their owners' backs.

Invocation carrier suboperations remain: opening request/terminal result, unary
child request/result, parent cancel/child cancel, and publication opening/ticks.
They are frames, not additional public RPCs. Keep catalog sections, requirements,
visibility, fingerprints, digest, descriptors and failure causes used in admission.

### Resulting Counts

| Count Boundary | Present | Proposed Candidate |
| --- | ---: | ---: |
| Mapping rows: plugin carriers plus built-ins | 45 = 4 + 41 | 43 = 4 + 39 |
| Dispositions of the 45 present rows | 45 | 4 kept, 37 renamed, 3 merged into 1, 1 removed |
| Newly required operation | 0 | 1: GetFileProduction for upload status/resume |
| Storage declarations | 34 | 32 = 34 − 2 from watch merge − 1 upload route + 1 status |
| Logical service groups when registered | 5 | 3: Plugin, Host, Storage |
| Routes at the stated exposure boundary | 64 = 4 + 30 ordinary + 30 managed | 39 = 4 + 35 canonical; no automatic twins |
| Static Storage subset | 11 of 34 | Generated from authoritative exposed declarations; no independent subset |
| StorageBackend hooks | 58 | 52 |
| StateBackend hooks | 8 | 8 |

The 39-route proposal replaces the exposed streaming WriteFile route with six
unary routes: CreateFileProduction, GetFileProduction, AppendFileBytes,
CreateFileWriter, FinishFileProduction and AbandonFileProduction. Five already
exist without gRPC bindings; status is new. Other native-only declarations stay
native-only, and all routes retain admission/grant checks. The row-watch merge
does not reduce the exposed route count: only FollowRecords is currently bound.
This is a candidate exposure choice, not a claim that every storage operation is
already served by the static service. Explicit managed exceptions consume one
canonical route each and do not inflate this count. Final message totals are
open until wrappers, typed discovery and reachable payload deletion are enumerated;
the research's arithmetic is not a substitute for that descriptor census.

### Complete Native Provider Mapping

These 66 present hooks are not additional RPCs. Proposed names below are semantic
names; implementations retain normal language casing. Private provider hooks
such as explicit flush remain private mechanisms, not public grammar exceptions.
Every unchanged guarantee remains required even when a hook is renamed or merged.

| StorageBackend Hook | Proposed Name | Disposition and Preserved Responsibility |
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
| finish_file | FinishFileProduction | Renamed; integrity and publication position. |
| abort_file | DeleteFileStaging | Renamed; unpublished staging cleanup. |
| create_production | CreateFileProduction | Renamed; resumable production. |
| fence_production | SetFileWriterGeneration | Renamed; atomic expected/next fencing. |
| save_production | SetFileProductionCheckpoints | Renamed; internal restart metadata, not a read family. |
| recover_productions | ListFileProductionCheckpoints | Renamed; bounded recovery metadata. |
| file_progress | GetFileProduction | Renamed; explicit saved/accepted progress. |
| read_production | GetFileProductionBytes | Renamed; retained production range. |
| abandon_file | AbandonFileProduction | Renamed; preserve partial-data guarantee. |
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

| StateBackend Hook | Proposed Name | Disposition and Preserved Responsibility |
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

### SDK Ownership Remains Open

The owner raised two alternatives: keep reusable library language SDKs, or let
product SDKs implement the plugin protocol directly. Neither is selected. Current
SDKs contain substantive protocol behavior, not merely protobuf bindings.

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
reconnect/restart and managed/remote stop behavior. These requirements survive
either SDK ownership choice. Deleting SDKs without replacement fixtures and
consumers would remove proof rather than simplify it. The library currently has
only one generic managed application client, in Rust; four-language convenience
client parity is not a landed fact or an approved new requirement.

### Defects to Fix Regardless

| Defect | Evidence | Proposed Independent Correction |
| --- | --- | --- |
| Python defaults to Public; other SDKs default to Private | `sdk/python/src/harmos/contracts.py`, Rust operation declarations, C# Operation and C++ header. | Align Python to Private and require explicit Public intent; explicit visibility everywhere remains a lock question. |
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
- Latest durable deliberately omits newer accepted data: measure observed lag,
  expose durability explicitly and retain writer barriers.
- Current follows can emit accepted data: changing them requires explicit durable
  boundaries and deterministic gap/reconnect proof, not a name-only edit.
- Unary sessions add round trips and remove the one-shot path on providers lacking
  sessions: document capability refusal and measure chunk overhead without changing
  consumer-owned concurrency/size budgets.
- Retained cuts and deletion can conflict: owner review must settle reader lifetime
  before removing or reclaiming content. No stored-data deletion is authorized here.

## Open Questions Before Lock

These questions remain open on purpose; the owner requested a proposal, not a
locked implementation inventory. Settle contract-changing questions before coding.

1. **SDK ownership:** keep library SDKs or direct protocol implementations in product
   SDKs? Which shared/generated components and conformance peers remain owned here?
2. **SDK promise:** if retained, is plugin semantic parity the promise, and must the
   Rust-only generic managed-client convenience exist in other languages? Must
   visibility be explicit everywhere, or is aligned Private default sufficient?
3. **Measured lag:** what lag and synchronous-commit contribution occur under the
   intended workload and persistence backlog? The approximately 100 ms plus commit
   expectation must be measured before the latest-durable recommendation is locked.
4. **Retained-reader deletion:** what happens to retained file readers after deletion
   across supported backends? Settle this before changing DeleteFile or reclamation.
5. **Issued cuts and restart:** how long do returned cuts/page tokens remain usable,
   what survives restart, and how are expiry, provenance and composite revision
   selections represented? Arbitrary earlier reads are rejected; retained issued
   selections still need explicit lifetimes and resynchronization rules. The product
   must decide retention of mutable presentation/context without moving it here.
6. **Watch contract:** approve snapshot/boundary-then-follow and durable-only saved
   data events; define issued-position/gap mapping for local file/directory and
   runtime-status sources without pretending external files are saved snapshots.
7. **Managed methods:** enumerate which methods actually promise cooperative final
   results. Preserve that capability; choose one route per method rather than
   deleting typed settlement or retaining universal twins.
8. **Exact message/service inventory:** approve wrapper payloads, typed catalog
   messages, namespace migration, canonical service grouping and six upload
   bindings; recalculate reachable messages from the locked descriptors.
9. **Cross-repository report previews:** the product's recommendation to create a
   preview report resource rather than treating publication as a read remains a
   product lock question; no report operations are added to this library.

Queued cancellation, single-resource rename, Get/Set spelling, no arbitrary
historical reads, and choosing unary sessions are recorded above rather than
reopened. Session retention, receipt expiry and backend support details still need
review within the corresponding contract questions.

## Migration Plan

First lock decisions, inventory and SDK ownership. Then change authoritative
protocol/generation and every affected library consumer together in compiling
slices, preserving existing data encodings. Reuse the existing production owner
for unary uploads and remove superseded routes/envelopes only after consumers
migrate. Update conformance fixtures and affected guides with each slice.

Publish/pin one exact library revision only under later authorization; the product
then adopts it, repairs its host gaps, and migrates desktop/CLI/generated clients
together with removal of superseded hosts. Rollback uses a matched prior host/SDK
revision; it is not an adapter layer or an implicit storage reset. Storage format
changes require a separate data-migration decision. Final validation is separate
from source closure and follows focused proof on both required native platforms.
