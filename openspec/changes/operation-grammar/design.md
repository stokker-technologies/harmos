# Operation Grammar Design

Status: locked by the owner on 2026-11-12
Boundary, managers and nested resources locked by the owner on 2026-11-12
Naming rule amended by the owner on 2026-11-12: bare verbs on the service's resource

Nothing here claims implementation.
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

- **Names (Owner):** a service names its resource with a plural noun, or a
  singular noun for one thing, such as Plugin. Methods acting on that resource
  are bare verbs. A part read in exactly one way keeps its noun on its parent:
  Executions.Steps and Executions.Parameters are bounded pages.
  A part with more than one read/change mode is a nested resource with its own
  bare verbs: Plugins.Settings.Get/Set, Executions.Progress.Get/Watch and
  Plots.Settings.Get/Set. This replaces the earlier verb-plus-part rule and flat
  compound services. Mixed-resource Storage retains distinguishing resource nouns
  (ListRows/WatchRows, GetObject/SetObject); these are independent storage resources,
  not differently named parts of one resource. It is not split merely for brevity.
  Always return a response wrapper, including empty replies. Within each package,
  messages are `<Service><Method>Request/Response`, such as SettingsGetRequest in
  product.v1.plugins. Package qualification disambiguates equal message names.
  Operation IDs derive mechanically from the complete service namespace and method.
- **Nested Wire Layout (Owner):** a nested service lives in package
  `<owner>.v1.<lowercase parent>`, where `<owner>` is the library or a product's
  namespace, with the part as service name. One proto file per service, in a
  folder per parent: product/v1/plugins/settings.proto declares
  product.v1.plugins.Settings; its Get method's full name is
  product.v1.plugins.Settings/Get. Deeper parents extend the package path the same
  way. Namespace-only parents need no empty service. A nested resource may have
  a different owner: the library declares harmos.v1.Plugins while a product
  declares product.v1.plugins.Settings on the same host endpoint. This is intended.
- **Client Roots (Owner):** each of Rust, Python, C# and C++ exposes the same
  hierarchy from service packages/names, with its normal language casing:

  ```python
  client.plugins.settings.get(request)
  client.executions.progress.watch(request)
  client.executions.start(request)
  ```

  A small generator in each SDK build consumes a common table produced from the
  descriptor set: owner namespace, version, parent path, service, method, wrappers
  and cardinality. Strip the owner/version prefix, append lowercase parent/service
  segments, and bind generated transport clients at those paths. Compose descriptor
  tables for library and product services into one root, so library Plugins and
  product plugins.Settings share the plugins node. Reject actual leaf collisions;
  preserve package-qualified message identities. No hand-written per-operation
  naming or forwarding layer. This four-language naming commitment does not settle
  generic managed-client execution coverage or plugin semantic parity.

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
  Executions.Set carrying the whole small writable part returned by Executions.Get;
  only that identified execution changes.
- **Domain verbs (Owner):** use a domain verb only where a standard verb would
  hide a side effect, and state its one meaning. Start activates; Stop finishes at
  a safe boundary; Cancel interrupts; Restart replaces an activation; Seal closes
  an evidence lifecycle; Publish commits prepared visibility; Import admits an
  external representation; Export produces an external representation; Invoke
  calls a declared operation; Advance materializes a projection frontier; Capture
  freezes external data. Storage.PublishFileProduction validates and publishes an upload;
  Storage.DeleteFileProduction abandons its unpublished session. Plugin lifecycle verbs keep
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
  accepted is not durable. When a call fails on the client, resending is the
  client's responsibility. The same request identifier and normalized payload
  return the original receipt; a changed payload under that identifier conflicts.
  Declare deduplication lifetime; expired keys
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
  observers. Server streaming is reserved for Watch semantics (including noun-only
  single-mode streamed parts); plugin protocol carriers
  retain their own streams. Get/List and transfers are bounded unary calls.

## Library Design

Choices in this section are Owner decisions, except the explicit open questions.
The shared vocabulary does not move application scheduling or metadata
ownership into the library.

### Boundary and Manager Ownership

**LOCKED, 2026-11-12:** if the library has a manager for generic machinery, that
machinery belongs in the library. A product owns its storage-layer definition
(schemas, indexes, projections and what latest means), its objects and their
operations, and who may do what. It composes library services directly on its
endpoint: clients call the library Plugins, access, transfer and status services
without product redeclarations or forwarding wrappers.

A product must not rebuild generic machinery using only the protocol crate.
That previously produced a second copy of plugin connection, dispatch, streams
and process launch. The runtime managers are the shared implementation boundary.

**Manager Rule (LOCKED):** introduce a manager where there is something live to
own: changing state, background work or a lifecycle. Plain operations over storage
need none. This is the single manager-selection rule for both owners.

| Library Manager | Owned Service or Mechanism | Existing Runtime Type / Direction |
| --- | --- | --- |
| Plugin manager | Plugins: managed/remote entries; Create, Get, List, Watch, Set, Restart, Delete; verified bundles, persistence, state and failures; Plugin carrier placement | Generic plugin-manager work is already being implemented; integrate it. The current guest adapter has PluginManager in guest/host/plugin/manager.rs, not proof the complete entry service has landed. |
| gRPC manager | Access and Access.Listener: listener Get/Set, pairing creation, certificate regeneration/key rotation; listener shutdown | GrpcManager in grpc/manager.rs exists. Access service contracts are added by this decision. |
| Storage manager | Storage, including the six production/session actions and ranged GetFileBytes shared by clients/plugins | StorageManager in storage/manager.rs exists. A product supplies storage definitions/providers, not a second upload engine. |
| Service manager | Status, Status.Logs and Status.TraceEvents; host identity, health, availability, logs, traces and diagnostics archive | ServiceManager in services/mod.rs exists. Add the status contracts and narrow product-fact registration here. |
| Operation manager | CatalogEntries, Invocations and host-only HostValues; operation registration, dispatch, invocation lifecycle and protocol callbacks | OperationManager in operations/manager/mod.rs exists; its inner Manager is not a second public owner. |
| Stream manager | Samples and Plugin.Samples publication/demand coordination | StreamManager in streams/manager.rs exists. Plugin protocol serving remains library-owned. |
| Service manager / host clock | Time.Get and Time.Sleep, backed by the authoritative runtime clock | Reuse existing service lifecycle and clock ownership; no second clock manager is required. |
| State manager | State lifecycle, replay and recovery; no new public grammar service | StateManager in state/manager.rs exists. |
| Record manager | Record registration and lifecycle; persistence through Storage | RecordManager in records/manager.rs exists. |

Evidence paths in this table are relative to crates/harmos-runtime/src/.
Existing types are evidence of orchestration homes, not claims that the new
client-facing services are implemented.

Status describes the host process. A narrow ServiceManager registration lets a
product contribute its version/identity facts and component health/availability
through typed providers with host-owned lifetime and failure reporting. It does
not let a product replace generic collection or create a competing status service.
Product-only overview, appearance and environment reads remain under Backend.
Diagnostics returns an archive content reference; bytes use Storage.GetFileBytes.

Settings and secrets are too opinionated for a generic library. A product owns
their stores, schemas, user-interface declarations, archives and operating-system
credential-store use. The library owns only a settings-declaration section of the
plugin catalog and host-only HostValues.Get: a plugin requests a declared value,
the host application's registered resolver supplies it under product policy.
No library settings/secret store, UI schema or client-facing settings service is
introduced. This closes the per-plugin settings/secrets ownership question.

| Product Orchestration Home | Responsibilities |
| --- | --- |
| Execution manager | Queueing, lifecycle, retries, Stop/Cancel, recovery; choose cycles here or a small cycle manager when splitting, using the simplest complete option. |
| Prompt manager | Pending operator questions and answer lifecycle. |
| Report manager | Atomic publication; rendering remains a pure function with no manager. |
| Capture manager, only if recordings remain | Recording lifecycle and retained results; do not create a manager for still-image transformations. |
| Operations over storage | Evidence, catalog, notes, aggregates, plots and documents; no manager. Settings/secrets keep their product-owned storage/policy. |

Replace the product's single host object/shared state module with these owners.
Each service method is thin: it maps to one operation file, which calls a manager
or storage. File capture and prepared-effect recovery retain their existing
execution/resource lifecycle owner; do not invent collection managers.

Executions.Progress.Get returns the current value, and Executions.Get includes
that same current progress. Executions.Progress.Watch streams it. A plugin produces
progress by calling a host-only Executions.Progress.Set operation; no client-facing
Set is declared or projected. Producer authority comes from its invocation scope,
not from exposing runtime-owned progress as client-writable metadata.

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
message TimeGetRequest {}
message TimeGetResponse { google.protobuf.Timestamp now = 1; }
message TimeSleepRequest { google.protobuf.Duration duration = 1; }
message TimeSleepResponse {}
```

```proto
message StorageListRowsRequest {
  QuerySource source = 1;
  ReadCut at = 2;
  repeated Transform transforms = 3;
  PageRequest page = 4;
}
message StorageListRowsResponse {
  repeated StoredRow rows = 1;
  ReadCut resolved_at = 2;
  string next_page_token = 3;
}
message StorageWatchRowsRequest { StorageListRowsRequest selection = 1; }
message StorageWatchRowsResponse {
  ReadCut resolved_at = 1;
  oneof event {
    RowsSnapshot snapshot = 2;
    RowsChange change = 3;
    ResyncRequired resync_required = 4;
    Position durable_progress = 5;
  }
}
message StorageAppendRecordsRequest {
  string parent = 1;
  string request_id = 2;
  repeated Record records = 3;
}
message StorageAppendRecordsResponse { Receipt receipt = 1; }
```

The rows union covers records, samples and object revisions with the same source
selector; no separate follow implementations survive. Filter progress and bounded
initial pages are candidate Watch details pending that task. Native
Arrow/index/projection mechanisms remain
distinct from application-level selection. Get/List return bounded unary pages
or ranges; callers repeat reads for further data. No streaming download remains.

Plugin.Samples observes publication demand; it has no historical List on the
plugin carrier. Retained sample reads belong to host storage. Do not manufacture
catalog watches without an actual change feed, and retain device-driven streams
alongside host-clock interval streams.

```proto
message StorageGetObjectRequest { string name = 1; ReadCut at = 2; }
message StorageGetObjectResponse {
  ObjectValue object = 1; // Complete writable value.
  bytes revision = 2;
  ReadCut resolved_at = 3;
}
message StorageSetObjectRequest {
  ObjectValue object = 1; // Complete writable value from GetObject.
  bytes expected_revision = 2;
  string request_id = 3;
  bool create_if_absent = 4;
}
message StorageSetObjectResponse {
  oneof outcome {
    AppliedObject applied = 1; // Value, revision and receipt.
    ObjectConflict conflict = 2;
  }
}
message InvocationsGetRequest { string name = 1; ReadCut at = 2; }
message InvocationsGetResponse { Invocation invocation = 1; ReadCut resolved_at = 2; }
message InvocationsWatchRequest { string name = 1; ReadCut at = 2; }
message InvocationsWatchResponse { Invocation invocation = 1; ReadCut resolved_at = 2; }
message InvocationsCancelRequest { string name = 1; string reason = 2; }
message InvocationsCancelResponse { Invocation invocation = 1; }
service Invocations {
  rpc Get(InvocationsGetRequest) returns (InvocationsGetResponse);
  rpc Watch(InvocationsWatchRequest) returns (stream InvocationsWatchResponse);
  rpc Cancel(InvocationsCancelRequest) returns (InvocationsCancelResponse);
}
service Time {
  rpc Get(TimeGetRequest) returns (TimeGetResponse);
  rpc Sleep(TimeSleepRequest) returns (TimeSleepResponse);
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
message StorageCreateFileProductionRequest {
  FileMetadata metadata = 1;
  string request_id = 2;
}
message StorageCreateFileProductionResponse { FileProduction production = 1; Receipt receipt = 2; }
message StorageGetFileProductionRequest { string name = 1; ReadCut at = 2; }
message StorageGetFileProductionResponse { FileProduction production = 1; ReadCut resolved_at = 2; }
message StorageAppendFileBytesRequest {
  string name = 1;
  bytes writer_token = 2;
  uint64 offset = 3;
  bytes data = 4;
  string request_id = 5;
}
message StorageAppendFileBytesResponse { FileProduction production = 1; Receipt receipt = 2; }
message StorageCreateFileWriterRequest { string name = 1; bytes expected_writer = 2; }
message StorageCreateFileWriterResponse { bytes writer_token = 1; FileProduction production = 2; }
message StoragePublishFileProductionRequest {
  string name = 1;
  bytes writer_token = 2;
  uint64 length = 3;
  bytes sha256 = 4;
  string request_id = 5;
}
message StoragePublishFileProductionResponse { FileReference file = 1; Receipt receipt = 2; }
message StorageDeleteFileProductionRequest { string name = 1; string request_id = 2; }
message StorageDeleteFileProductionResponse { FileProduction production = 1; Receipt receipt = 2; }
```

```proto
message StorageGetFileBytesRequest {
  string name = 1;
  uint64 offset = 2;
  uint64 max_bytes = 3;
  ReadCut at = 4;
}
message StorageGetFileBytesResponse {
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

Every current logical operation appears once and maps to exactly one destination.
The removed streaming-upload convenience maps to the shared session entry point;
its subsequent workflow calls are not additional destinations. Present types use the current
request and final reply; current stream item types are noted in the reason. Locked
request/reply names always follow the method-wrapper rule. All target names and
dispositions are Owner decisions, including retained names whose payloads change.

Evidence key: paths below are under `crates/harmos-runtime/tests/storage/` for
storage tests, under `crates/harmos-operations/src/` for declarations, and under
`crates/harmos-runtime/src/plugin/grpc/` for plugin-host callers. Named test sources
are invocation evidence, not claims that tests were run in this authoring task.

| Present Operation | Present Request → Final Reply | Locked Operation | Destination Owner | Disposition and Reason / Caller Evidence |
| --- | --- | --- | --- | --- |
| Plugin.Describe | DescribeRequest → Description | Plugin.Get | Library | Renamed; connection.rs and SDK servers use identity/catalog admission. |
| Plugin.Invoke | HostFrame → PluginFrame | Plugin.Invoke | Library | Kept; invoke.rs; duplex control carrier still conveys one unary operation attempt. |
| Plugin.Publish | PublishFrame → Samples | Plugin.Samples | Library | Renamed; publish.rs; host-clock ticks and device publication remain distinct. |
| Plugin.Stop | StopRequest → StopResponse | Plugin.Stop | Library | Kept; connection.rs shutdown invokes cleanup. |
| host.catalog | Catalog → CatalogPage | CatalogEntries.List | Library | Renamed; sdk/rust/src/client.rs calls bounded discovery. |
| host.operations.status | Status → InstanceState | Invocations.Get | Library | Renamed; tests/operations/host.rs invokes owned status. |
| host.operations.cancel | Cancel → InstanceState | Invocations.Cancel | Library | Renamed; same host test; cancellation is an action. |
| host.operations.watch | Watch → InstanceState | Invocations.Watch | Library | Renamed; same host test; retain status events. |
| harmos.clock.now | Now → Time | Time.Get | Library | Renamed; four SDK contexts call the host clock. |
| harmos.clock.sleep | Sleep → TimeSleepResponse | Time.Sleep | Library | Kept; four SDK contexts; locked clock action, not WaitTime. |
| host.publish | Publish → Published | Samples.Append | Library | Renamed; streams/publish.rs and sdk/rust/src/wasm/context.rs publish immutable samples. |
| storage.status | StorageStatus → StorageInfo | Storage.Info | Library | Renamed; generated_status.rs. |
| storage.capabilities | Capabilities → StorageCapabilities | Storage.Capabilities | Library | Renamed; generated_native_journey.rs. |
| storage.open_snapshot | OpenSnapshot → SavedSnapshot | Storage.CreateSnapshot | Library | Renamed; same journey; retained backend read lease, not a second read family. |
| storage.release_snapshot | ReleaseSnapshot → Empty | Storage.DeleteSnapshot | Library | Renamed; same journey; release the lease. |
| storage.scan_arrow | ScanArrow → ArrowPage | Storage.ListColumnRows | Library | Renamed; same journey; preserve bounded physical work. |
| storage.scan_index | ScanIndex → IndexPage | Storage.ListIndexEntries | Library | Renamed; same journey; ordered seek semantics remain. |
| storage.register_index | RegisterIndex → DefinitionRevision | Storage.SetIndexDefinition | Library | Renamed; same journey; whole definition and expected revision from GetIndexDefinition; rebuild preconditions remain. |
| storage.register_projection | RegisterProjection → DefinitionRevision | Storage.SetProjectionDefinition | Library | Renamed; same journey; whole definition and expected revision from GetProjectionDefinition. |
| storage.advance_projection | AdvanceProjection → DefinitionRevision | Storage.AdvanceProjection | Library | Kept; same journey; side effect is checked materialization. |
| storage.read_projection | ReadProjection → ProjectionPage | Storage.ListProjectionRows | Library | Renamed; same journey; exact revision/frontier availability. |
| storage.inspect_projection | InspectProjection → ProjectionState | Storage.GetProjectionStatus | Library | Renamed; same journey; derived status is not writable. |
| storage.compare_exchange_object | CompareExchangeObject → CompareExchangeOutcome | Storage.SetObject | Library | Renamed; same journey; whole writable object and expected revision from GetObject; atomic conditional outcome. |
| storage.query | Query → QueryCompletion | Storage.ListRows | Library | Renamed; generated_query_batches.rs and generated_query_projection.rs; bounded unary pages replace QueryOutput streaming. |
| storage.follow_records | FollowRecords → StorageCursor | Storage.WatchRows, records selector | Library | Merged; generated_record_follow.rs; shared boundary/change union. |
| storage.follow_stream | FollowStream → StorageCursor | Storage.WatchRows, samples selector | Library | Merged; generated_stream_follow.rs; add consistent initial durable boundary. |
| storage.follow_objects | FollowObjects → StorageCursor | Storage.WatchRows, object-revision selector | Library | Merged; generated_object_follow.rs; preserve revision data. |
| storage.write_schema | WriteSchema → StorageCursor | Storage.SetSchema | Library | Renamed; operation_schemas.rs; whole small writable schema and expected revision from GetSchema; immutable revision rules remain. |
| storage.write_records | WriteRecords → WriteCompletion | Storage.AppendRecords | Library | Renamed; generated_record_exchange.rs; bounded unary batch replaces DataBatch/WriteReceipt exchange. |
| storage.write_streams | WriteStreams → WriteCompletion | Storage.AppendSamples | Library | Renamed; generated_exchange_cancellation.rs; preserve admitted outcomes with receipts. |
| storage.write_objects | WriteObjects → WriteCompletion | Storage.AppendObjectRevisions | Library | Renamed; generated_object_exchange.rs; bulk ingestion does not replace atomic SetObject. |
| storage.write_file | WriteFile → FileReference | Storage.CreateFileProduction | Library | Removed; generated_file_upload.rs; maps to the session entry point, then Storage.AppendFileBytes and Storage.PublishFileProduction replace the streaming convenience workflow. |
| storage.create_file | CreateFile → FileProducer | Storage.CreateFileProduction | Library | Renamed; generated_file_mutations.rs; create the upload/production session. |
| storage.append_file | AppendFile → FileProducer | Storage.AppendFileBytes | Library | Renamed; same mutations; bounded immutable chunks. |
| storage.resume_file | ResumeFile → FileProducer | Storage.CreateFileWriter | Library | Renamed; same mutations; fence the previous writer. |
| storage.finish_file | FinishFile → FileReference | Storage.PublishFileProduction | Library | Renamed; same mutations; durable final reference. |
| storage.abandon_file | AbandonFile → FileProgress | Storage.DeleteFileProduction | Library | Renamed; same mutations; Delete of the unpublished session abandons it. |
| storage.delete_file | DeleteFile → StagedFile | Storage.DeleteFile | Library | Kept; same mutations; retained-reader decision remains open. |
| storage.read_file | ReadFile → FileCompletion | Storage.GetFileBytes | Library | Renamed; generated_read.rs; streaming download removed in favor of bounded unary ranges with offset, maximum bytes, total length and digest. |
| storage.hash_file | HashFile → FileHash | Storage.GetFileHash | Library | Renamed; examples/schematic/operations/src/transfer/hash.rs and guest SDK call it. |
| storage.capture_file | CaptureFile → FileReference | Storage.CaptureFile | Library | Kept; generated_capture.rs; independent captured bytes. |
| storage.follow_file | FollowFile → FileCompletion | Storage.WatchFileBytes | Library | Renamed; generated_capture.rs; FileEvent preserves replacement/truncation. |
| storage.follow_production | FollowProduction → FileProgress | Storage.WatchFileProduction | Library | Renamed; generated_reconnect.rs and generated_future_offset.rs; writer fencing and retained chunks remain. |
| storage.list_directory | ListDirectory → DirectoryCompletion | Storage.ListDirectoryEntries | Library | Renamed; generated_directory.rs; bounded DirectoryEntry selection. |
| storage.watch_directory | WatchDirectory → DirectoryCompletion | Storage.WatchDirectoryEntries | Library | Renamed; same directory test; preserve DirectoryEvent and rescan-required semantics. |
Samples.Append and Storage.AppendSamples have separate namespaces: the former
publishes a declared stream, the latter admits persisted sample batches. They are
not merged. Locked library namespaces include their service, for example
`harmos.storage.append_records`; the exact namespace transition and fingerprints
migrate together. Public names in the table do not rename arbitrary
consumer-defined operation IDs behind their owners' backs.

Invocation carrier suboperations remain: opening request/terminal result, unary
child request/result, parent cancel/child cancel, and publication opening/ticks.
They are frames, not additional public RPCs. Keep catalog sections, requirements,
visibility, fingerprints, digest, descriptors and failure causes used in admission.

### Added Get Mirrors

| Added Library Operation (Owner: Library) | Exposure | Locked Value Contract |
| --- | --- | --- |
| Storage.GetIndexDefinition | Storage RPC | Whole writable index definition and revision, mirrored by SetIndexDefinition. |
| Storage.GetProjectionDefinition | Storage RPC | Whole writable projection definition and revision, mirrored by SetProjectionDefinition. |
| Storage.GetObject | Storage RPC | Whole writable object and revision, mirrored by SetObject; explicit absence for creation. |
| Storage.GetSchema | Native-only, matching SetSchema | Whole writable schema and revision, mirrored by SetSchema. |
| Storage.GetFileProduction | Storage RPC | Accepted/durable upload progress, writer generation and resume status. |

These small writable parts need no subdivision. Large product configuration or
artifact metadata must instead map to separately settable named sub-resources,
each with its own Get/Set and revision, rather than a monolithic replacement.
Product rename/archive setters map to Executions.Get/Executions.Set of the whole small
writable execution part; rename affects only the selected execution. These product
methods are not added to the library counts.

### Added Generic Machinery Inventory

All destinations here are Library-owned. The first six rows add 19 client methods
to the earlier 42-method candidate; HostValues adds one host-only operation.
The already-running plugin-manager implementation supplies Plugins; this inventory
does not direct a second implementation. Transfers reuse Storage, adding no parallel
Content/Uploads service or upload state.

| Service | Methods | Exposure / Value Contract |
| --- | --- | --- |
| Plugins | Create, Get, List, Watch, Set, Restart, Delete | Client gRPC; managed or remote entries, verified bundle references, persistence, activation state and structured failures. Get/Set mirror the whole writable entry and revision. |
| Access | CreatePairing, CreateCertificate | Client gRPC; pairing admission and validated certificate regeneration/rotation under GrpcManager. |
| Access.Listener | Get, Set | Client gRPC; whole writable listener value and expected revision, with runtime facts returned separately. |
| Status | Identity, Health, Availability, Diagnostics | Client gRPC; noun-only single-mode host reads; diagnostics archive reference uses existing ranged content reads. |
| Status.Logs | List, Watch | Client gRPC; one bounded log selector and position vocabulary. |
| Status.TraceEvents | List, Watch | Client gRPC; one bounded trace-event selector and position vocabulary. |
| HostValues | Get | Host-only operation over the plugin invocation carrier; generic declared-value resolution, product-owned policy/resolver, no client gRPC service. |

### Service Regrouping

Host becomes CatalogEntries, Invocations, Time and native-only Samples.
Plugin is singular for its own Get, Invoke, Samples and Stop; Plugin.Samples is
one streamed publication-demand part, not a manufactured List/Watch pair.
Storage remains the mixed-resource service under its existing contract: Info and
Capabilities are noun-only parts; its other methods distinguish separate storage
resources. The nested-part rule does not invent a generic Storage settings store.

The added services are Plugins, Access, Access.Listener, Status, Status.Logs,
Status.TraceEvents and native-only HostValues. Nested services use their parent's
subpackage, not a compound top-level name. A product may add plugins.Settings and
storage.Sources in its own namespace beside library services.

### Resulting Counts by Owner

Counts are templates, not resource instances. A logical service is a namespace
with operations; a namespace-only parent is not an empty service. A gRPC service
has at least one client/carrier method. Host/plugin-only operations do not gain
client gRPC methods merely because their logical namespace also has client reads.

| Owner / Boundary | Services (Nested Included) | Operations | gRPC Methods | Requests | Responses |
| --- | ---: | ---: | ---: | ---: | ---: |
| Library, present plugin carriers and built-ins | 5 logical | 45 | 64, including 30 automatic twins | No uniform wrapper rule | No uniform wrapper rule |
| Library, locked all operations | 13 logical, 3 nested | 67 | 61 | 67 | 67 |
| Library, exposed subset | 11 gRPC, 3 nested | 61 | 61 | 61 | 61 |
| Library, native/host-only subset | Samples and HostValues; 4 further native Storage operations | 6 | 0 | 6 | 6 |
| Product, present client surface | 23 gRPC | 132 ordinary | 264, including 132 automatic twins | Not the target wrapper census | Not the target wrapper census |
| Product, locked client subset | 32 gRPC, 20 nested | 92 | 92 | 92 | 92 |
| Product, additional host/plugin-only templates | 7 additional logical services, 2 nested; 8 methods on shared logical namespaces | 18 | 0 | 18 | 18 |
| Product, locked all operations | 39 logical, 22 nested | 110 | 92 | 110 | 110 |

Library arithmetic: 45 present operations yield 42 surviving destinations
(6 method spellings retained, 35 renamed, 3 follow operations merged into one,
1 streaming upload convenience removed); 5 Get companions yield 47.
Add 7 Plugins + 4 Access + 8 Status + 1 HostValues = 67.
Exposed methods: 42 + 7 + 4 + 8 = 61. Six native/host-only operations are
Samples.Append, Storage.Info, Storage.SetSchema, Storage.GetSchema,
Storage.DeleteFile and HostValues.Get. Storage remains 36 declarations,
32 exposed and 4 native-only. StorageBackend remains 58 to 52 hooks and
StateBackend 8 to 8, for 66 present / 60 target provider hooks.

Library messages: 134 = 67 requests + 67 responses, including 122 exposed
and 12 native/host-only. Product messages: 220 = 110 requests + 110 responses,
including 184 for client methods and 36 additional host/plugin-only wrappers.
Shared methods used by plugins are counted once. Neither count includes domain
payloads, shared primitives, nested event/control frames or imported protobuf
messages; the implementation descriptor inventory must census those separately.

The library's earlier exposed arithmetic remains 64 - 30 twins - 1 WriteFile
+ 6 production/session bindings + 3 exposed Get mirrors = 42.
All six session methods are exposed. ReadFile is replaced in place by ranged
GetFileBytes; WatchFileBytes preserves growth/replacement/truncation observation.

The product's earlier 118-method candidate gives 92 after removing 7 Plugins,
4 Access, 8 host-status and 7 content methods now supplied by the library.
Those 7 content methods are 5 upload actions, ranged bytes Get and identified-file
bytes Watch. Product artifact/file metadata and collection watches remain.
This does not add 26 methods to the library: transfer methods already exist there.
No client progress Set is included. Product host/plugin-only counts enumerate
18 distinct additional templates below, rather than adding 24 mapping rows to 92.

No automatic twins remain. Only Watch semantics (including single-mode streamed
parts), plugin carriers and explicitly promised cooperative final-result
replacements stream. Such a replacement changes no method count.

### Complete Product Client Mapping

This generic handoff inventory is derived from the sibling change's 132 present
ordinary client methods, read without editing that checkout. All 132 automatic
Run<Method> twins are removed and map to the same canonical destination as their
ordinary row; the table therefore accounts for all 264 current gRPC methods.
Each present operation has exactly one canonical destination and owner. Required
follow-up resource workflows are explained in the reason, not additional or
alternative mapping destinations. There is no new aggregate compatibility RPC.

Callers: G = desktop/native bridge, C = CLI/client, S = SDK convenience/host
story, T = integration story, N = no named production caller found (not dead).
Product source paths and names are intentionally omitted; operation identifiers
below use generic resource vocabulary.

| Present Operation | Locked Destination | Destination Owner | Disposition / Preserved Workflow | Callers |
| --- | --- | --- | --- | --- |
| `Appearance.GetAppearance` | Backend.Appearance | Product | Merged: Move appearance ownership. | G |
| `Backend.CollectDiagnostics` | Status.Diagnostics | Library | Renamed: Read bounded diagnostic output. | G |
| `Backend.GetAvailability` | Status.Availability | Library | Renamed: Retain availability. | CS |
| `Backend.GetHealth` | Status.Health | Library | Merged: One health owner. | CS |
| `Backend.GetIdentity` | Status.Identity | Library | Merged: One identity owner. | GCS |
| `Backend.WatchLogs` | Status.Logs.Watch | Library | Kept: Durable log watch. | T |
| `Backend.WatchTrace` | Status.TraceEvents.Watch | Library | Renamed: Name trace event resource. | G |
| `Catalog.GetRecordSchema` | Catalog.RecordSchemas.Get | Product | Merged: Catalog or frozen execution parent. | N |
| `Catalog.GetSuite` | Catalog.Suites.Get | Product | Kept: Retain suite definition. | GCS |
| `Catalog.GetTelemetrySchema` | Catalog.StreamSchemas.Get | Product | Renamed: Use stream vocabulary. | N |
| `Catalog.ListRecordSchemas` | Catalog.RecordSchemas.List | Product | Merged: Catalog or frozen execution parent. | N |
| `Catalog.ListSuites` | Catalog.Suites.List | Product | Kept: Retain suite catalog. | GCS |
| `Catalog.ListTelemetrySchemas` | Catalog.StreamSchemas.List | Product | Merged: Use stream vocabulary. | N |
| `Content.DownloadAppearanceAsset` | Storage.GetFileBytes | Library | Merged: Bounded bytes by content identity. | G |
| `Content.DownloadArtifact` | Content.Artifacts.Get | Product | Then use the returned immutable reference with library Storage.GetFileBytes. | GC |
| `Content.DownloadEvidence` | Storage.GetFileBytes | Library | Merged: Ranged archive content; preserve scope, cut and digest verification. | C |
| `Content.DownloadExecutionFile` | Content.ExecutionFiles.Get | Product | Then use library Storage.GetFileBytes for bounded content ranges. | C |
| `Content.FollowExecutionFile` | Storage.WatchFileBytes | Library | Direct library bytes Watch preserves growth, replacement and truncation; product Content.ExecutionFiles.Watch remains the metadata/collection watch. | G |
| `Content.ListArtifacts` | Content.Artifacts.List | Product | Merged: Retain bounded artifact selection. | N |
| `Content.ListExecutionFiles` | Content.ExecutionFiles.List | Product | Merged: Retain bounded file selection. | G |
| `Content.ReadExecutionFile` | Content.ExecutionFiles.Get | Product | Merged: Same file resource, text/byte ranges. | G |
| `Content.UploadArtifact` | Content.Artifacts.Create | Product | First use library Storage.CreateFileProduction, AppendFileBytes, GetFileProduction, CreateFileWriter as needed, PublishFileProduction or DeleteFileProduction; Create registers the product artifact reference. | GC |
| `Documents.CaptureCurrentDocument` | Documents.Get | Product | Merged: Evaluated output without publication. | G |
| `Documents.UpdatePlotSettings` | Plots.Settings.Set | Product | Renamed: Whole small writable plot value and expected revision from Plots.Settings.Get. | G |
| `Evidence.GetCapturedRecordSchema` | Catalog.RecordSchemas.Get | Product | Merged: Execution parent selects frozen schema. | N |
| `Evidence.ListRecords` | Evidence.Records.List | Product | Merged: One selector/cut contract. | GC |
| `Evidence.WatchRecords` | Evidence.Records.Watch | Product | Kept: Snapshot then durable changes. | GC |
| `Execution.AppendExecutions` | Executions.Create | Product | Renamed: Allocate mutable execution resources. | GC |
| `Execution.CancelExecution` | Executions.Cancel | Product | Renamed: Running only, queued refusal names action. | G |
| `Execution.CreateCycle` | Cycles.Create | Product | Merged: Preserve new-cycle admission. | GCS |
| `Execution.DeleteCase` | Cases.Delete | Product | Renamed: Preserve lifecycle admission. | G |
| `Execution.GetCase` | Cases.Get | Product | Renamed: Retain planned case. | G |
| `Execution.GetCycle` | Cycles.Get | Product | Merged: One operational/saved read. | GC |
| `Execution.GetExecution` | Executions.Get | Product | Merged: One operational/saved read with seal. | GCS |
| `Execution.GetExecutionCommand` | Executions.Commands.Get | Product | Renamed: Retained command receipt. | C |
| `Execution.ListCaseParameters` | Cases.Parameters | Product | Renamed: Pinned definition values. | G |
| `Execution.ListCases` | Cases.List | Product | Renamed: Retain ordered cases. | G |
| `Execution.ListCycles` | Cycles.List | Product | Merged: Operational/query/history selection. | G |
| `Execution.ListExecutionParameters` | Executions.Parameters | Product | Renamed: Frozen execution values. | G |
| `Execution.ListExecutionSteps` | Executions.Steps | Product | Renamed: Frozen steps and state. | G |
| `Execution.ListExecutions` | Executions.List | Product | Merged: Operational/query/history selection. | GC |
| `Execution.PauseCycle` | Cycles.Set | Product | Merged: Whole small writable value and Get revision; preserve pause admission. | S |
| `Execution.PauseExecution` | Executions.Set | Product | Merged: Whole small writable value and Get revision; preserve pause admission. | GC |
| `Execution.RenameCycle` | Cycles.Set | Product | Merged: Whole small writable value and Get revision, including label. | G |
| `Execution.RenameExecution` | Executions.Set | Product | Merged: Whole small writable value and Get revision; rename only this execution. | G |
| `Execution.RerunCycle` | Cycles.Create | Product | Merged: source_cycle preserves rerun notices. | GC |
| `Execution.ResumeCycle` | Cycles.Set | Product | Merged: Whole small writable value and Get revision, with paused=false. | S |
| `Execution.ResumeExecution` | Executions.Set | Product | Merged: Whole small writable value and Get revision, with paused=false. | GC |
| `Execution.SealCycle` | Cycles.Seal | Product | Renamed: Preserve durable finalization. | GC |
| `Execution.SetCycleArchived` | Cycles.Set | Product | Merged: Whole small writable value and Get revision, including archived. | G |
| `Execution.SetExecutionArchived` | Executions.Set | Product | Merged: Whole small writable value and Get revision, including archived. | G |
| `Execution.StartExecution` | Executions.Start | Product | Renamed: Activate selected execution. | GC |
| `Execution.StopExecution` | Executions.Stop | Product | Renamed: Stop scheduling at safe boundary. | GC |
| `Execution.SubmitExecutionCommand` | Executions.Commands.Create | Product | Renamed: Interactive Next/RunStep/Finish, replay receipt. | C |
| `Execution.UpdateCyclePolicy` | Cycles.Set | Product | Merged: Whole small writable cycle value and Get revision, including policy. | G |
| `Health.ListHealth` | Evidence.HealthEvents.List | Product | Merged: Name evidence collection. | GS |
| `History.BrowseCycles` | Cycles.List | Product | Merged: Retain saved filtering and ordering. | G |
| `History.BrowseExecutions` | Executions.List | Product | Merged: Retain live/saved availability. | G |
| `History.DeleteExecution` | Executions.Delete | Product | Renamed: Keep seal precondition and replay protection. | G |
| `History.DownloadCheckpointArtifact` | Content.Artifacts.Get | Product | Select metadata at the returned cut/seal, then use library Storage.GetFileBytes. | G |
| `History.GetCheckpointContext` | Executions.Get | Product | Returns the issued cut and resource references; read Cycles.Get, Documents.Get, Prompts.List, Evidence.HealthEvents.List, Evidence.Notes.List, Evidence.Traceability.Get and Backend.Appearance at that cut. No checkpoint DTO or aggregate wrapper is added. | GC |
| `History.GetCheckpointDocument` | Documents.Get | Product | Merged: Same read at returned cut/seal. | GC |
| `History.GetCheckpointEvidence` | Content.Artifacts.List | Product | Use the same cut for Content.ExecutionFiles.List, Catalog.RecordSchemas.List and Catalog.StreamSchemas.List; frozen availability and schemas are preserved. | G |
| `History.GetExecutionCheckpoint` | Executions.Get | Product | Merged: Return position and seal in ordinary read. | C |
| `History.ListCheckpointRecords` | Evidence.Records.List | Product | Merged: Same selector at returned cut/seal. | G |
| `History.ListCheckpointTelemetry` | Evidence.Samples.List | Product | Merged: Same selector at returned cut/seal. | G |
| `History.ReadCheckpointOutput` | Content.ExecutionFiles.Get | Product | Merged: Same bounded file read at cut/seal. | G |
| `Media.AudioRecording` | Media.Captures.Create | Product | Follow with Media.Captures.Watch/Get/Stop for the retained recording. | G |
| `Media.CameraRecording` | Media.Captures.Create | Product | Follow with Media.Captures.Watch/Get/Stop; preserve unsupported-kind refusal. | N |
| `Media.Photo` | Media.Captures.Create | Product | Merged: Typed photo source. | G |
| `Media.ScreenRecording` | Media.Captures.Create | Product | Follow with Media.Captures.Watch/Get/Stop; preserve unsupported-kind refusal. | N |
| `Media.Screenshot` | Media.Captures.Create | Product | Merged: Typed screenshot source. | G |
| `Notes.CreateNote` | Evidence.Notes.Create | Product | Kept: Allocate identified note. | G |
| `Notes.EditNote` | Evidence.Notes.Set | Product | Renamed: Whole note writable value and revision from Evidence.Notes.Get. | G |
| `Notes.ListNotes` | Evidence.Notes.List | Product | Merged: Retain note collection. | G |
| `Observation.GetRuntimeShell` | Backend.Overview.Get | Product | Renamed: Runtime overview resource. | G |
| `Observation.GetTraceability` | Evidence.Traceability.Get | Product | Merged: Retain evidence context. | G |
| `Observation.WatchExecutionProgress` | Executions.Progress.Watch | Product | Renamed: Paired progress read/watch. | GS |
| `Observation.WatchRuntime` | Backend.Overview.Watch | Product | Renamed: Same overview selection. | G |
| `Planning.MaterializeTemplate` | Cases.Create | Product | Merged: Single materialized selection. | GCS |
| `Planning.PreviewSelection` | Cases.Create | Product | Merged: validate_only, no registration. | GC |
| `Planning.RegisterSelection` | Cases.Create | Product | Merged: Atomic plan with replay key. | C |
| `Plots.ListAnalyses` | Catalog.Analyses | Product | Renamed: Move immutable declarations. | N |
| `Plots.QuerySnapshots` | Plots.Snapshots.List | Product | Renamed: Bounded evaluated snapshots. | G |
| `Plots.Watch` | Plots.Snapshots.Watch | Product | Renamed: Same plot selector/cut. | G |
| `Plugins.GetInstallation` | Plugins.Get | Library | Renamed: One managed/remote entry. | G |
| `Plugins.Install` | Plugins.Create | Library | First transfer/verify the bundle through the same library Storage session; Plugins.Create admits that verified bundle. | GC |
| `Plugins.ListAdmittedPlugins` | Plugins.List | Library | Merged: state=READY filter. | G |
| `Plugins.ListInstallations` | Plugins.List | Library | Merged: One entry selection. | G |
| `Plugins.Reload` | Plugins.Restart | Library | Renamed: Replace activation, retain finalization rules. | GS |
| `Plugins.Remove` | Plugins.Delete | Library | Renamed: Remove entry, migrate old payload. | G |
| `Plugins.SetEnabled` | Plugins.Set | Library | Renamed: Whole small plugin writable value and Plugins.Get revision, including enabled. | G |
| `Plugins.List` | Plugins.List | Library | Merged: One managed/remote entry list. | S |
| `Plugins.Add` | Plugins.Create | Library | Merged: Remote placement admission. | S |
| `Prompts.GetDefinition` | Catalog.PromptDefinitions.Get | Product | Renamed: Separate definition from asked prompt. | S |
| `Prompts.GetPrompt` | Prompts.Get | Product | Renamed: Retain asked prompt state. | GCS |
| `Prompts.ListDefinitions` | Catalog.PromptDefinitions.List | Product | Renamed: Immutable definitions. | S |
| `Prompts.ListPending` | Prompts.List | Product | Merged: state=PENDING filter. | GCS |
| `Prompts.Respond` | Prompts.Answers.Create | Product | Renamed: One answer resource and replay receipt. | GCS |
| `Query.Aggregate` | Aggregates.Get | Product | Renamed: Complete read-only aggregate. | T |
| `Query.AggregateSamples` | Evidence.Statistics | Product | Renamed: Retain stream statistics selectors. | T |
| `Query.ListCycles` | Cycles.List | Product | Merged: Preserve projection/filter semantics. | T |
| `Query.ListExecutions` | Executions.List | Product | Merged: Preserve projection/filter semantics. | TS |
| `Query.ListRecords` | Evidence.Records.List | Product | Merged: One canonical evidence read. | T |
| `Query.ListSamples` | Evidence.Samples.List | Product | Merged: One canonical sample read. | T |
| `Remote.CreatePairing` | Access.CreatePairing | Library | Kept: Preserve access material lifecycle. | G |
| `Remote.ReadListener` | Access.Listener.Get | Library | Renamed: Derived listener state. | G |
| `Remote.RegenerateCertificate` | Access.CreateCertificate | Library | Renamed: Allocate replacement certificate. | G |
| `Remote.UpdateListener` | Access.Listener.Set | Library | Renamed: Whole listener writable settings and revision from Access.Listener.Get. | G |
| `Reports.PublishExecutionReport` | Reports.Publish | Product | Renamed: Explicit durable publication, never implicit download. | S |
| `Reports.DownloadCycleReport` | Reports.Get | Product | Returns a published content reference or explicit preview-needed outcome; Reports.Create renders an explicit preview, and library Storage.GetFileBytes reads either reference. Publication remains Reports.Publish. | GCS |
| `Reports.DownloadExecutionReport` | Reports.Get | Product | Returns a published content reference or explicit preview-needed outcome; Reports.Create renders an explicit preview, and library Storage.GetFileBytes reads either reference. Publication remains Reports.Publish. | GCS |
| `Reports.MaterializeReport` | Reports.Create | Product | Merged: Render only, no publication. | N |
| `Runtime.GetHealth` | Status.Health | Library | Removed: Remove duplicate health gRPC method. | C |
| `Runtime.GetIdentity` | Status.Identity | Library | Removed: Remove duplicate identity gRPC method. | C |
| `Secrets.Export` | Secrets.Export | Product | Renamed: Existing Base64 JSON transfer, not encryption. | G |
| `Secrets.Import` | Secrets.Import | Product | Merged: Preserve choices and generation conflict. | G |
| `Secrets.List` | Secrets.List | Product | Renamed: Bounded metadata, not secret values. | G |
| `Secrets.Lookup` | Secrets.Get | Product | Renamed: Granted identifier only. | N |
| `Secrets.Preview` | Secrets.Import | Product | Merged: validate_only, no writes. | G |
| `Secrets.Put` | Secrets.Set | Product | Renamed: Whole writable secret value and revision from Secrets.Get. | G |
| `Secrets.Remove` | Secrets.Delete | Product | Renamed: Preserve keyring/catalog cleanup. | G |
| `Settings.GetPluginSettings` | Plugins.Settings.Get | Product | Renamed: Saved values, not UI declarations. | G |
| `Settings.GetSettings` | Settings.Schema | Product | Renamed: UI declarations/cards/bootstrap metadata. | G |
| `Settings.ListStorageSources` | Storage.Sources.Get | Product | Renamed: One ordered value. | G |
| `Settings.ReadApplicationSettings` | Settings.Get | Product | Renamed: Revisioned application values. | G |
| `Settings.ReorderStorageSources` | Storage.Sources.Set | Product | Merged: Whole small ordered value and revision from Storage.Sources.Get. | G |
| `Settings.SetStorageSourceEnabled` | Storage.Sources.Set | Product | Merged: Whole small ordered value and Get revision, including enabled state. | G |
| `Settings.UpdateApplicationSettings` | Settings.Set | Product | Renamed: Whole writable value and matching Get revision. | G |
| `Settings.UpdatePluginSettings` | Plugins.Settings.Set | Product | Renamed: Whole writable value and matching Get revision; if large, use separately revisioned setting sub-resources with Plugins.Settings.Get/Plugins.Settings.Set. | G |
| `Telemetry.ListSamples` | Evidence.Samples.List | Product | Merged: One canonical sample read. | CS |
| `Telemetry.WatchSamples` | Evidence.Samples.Watch | Product | Kept: Same source selector and durable boundary. | C |

### Product Client Service Inventory

Every row is Product-owned, declared in its namespace. No Plugins, Access,
Status or transfer implementation is redeclared. Namespace-only parents Content,
Plots, Media and Storage need no empty product service. A product owns
plugins.Settings beside the library Plugins, and storage.Sources describes its
own storage-source policy rather than library transfer machinery.

| Service | Nested | Methods | Count |
| --- | --- | --- | ---: |
| Backend | No | Appearance | 1 |
| Backend.Overview | Yes | Get, Watch | 2 |
| Catalog.Suites | Yes | Get, List | 2 |
| Catalog.RecordSchemas | Yes | Get, List | 2 |
| Catalog.StreamSchemas | Yes | Get, List | 2 |
| Catalog.PromptDefinitions | Yes | Get, List | 2 |
| Catalog | No | Analyses | 1 |
| Cases | No | Create, Get, List, Watch, Delete, Parameters | 6 |
| Cycles | No | Get, List, Watch, Create, Set, Seal | 6 |
| Executions | No | Get, List, Watch, Create, Set, Start, Stop, Cancel, Delete, Parameters, Steps | 11 |
| Executions.Commands | Yes | Get, Create | 2 |
| Executions.Progress | Yes | Get, Watch | 2 |
| Evidence.Records | Yes | List, Watch | 2 |
| Evidence | No | Statistics | 1 |
| Evidence.Traceability | Yes | Get, Watch | 2 |
| Evidence.Notes | Yes | List, Watch, Get, Create, Set | 5 |
| Evidence.HealthEvents | Yes | List, Watch | 2 |
| Evidence.Samples | Yes | List, Watch | 2 |
| Content.Artifacts | Yes | Get, Create, List, Watch | 4 |
| Content.ExecutionFiles | Yes | Get, List, Watch | 3 |
| Documents | No | Get, Watch | 2 |
| Plots.Snapshots | Yes | List, Watch | 2 |
| Plots.Settings | Yes | Get, Set | 2 |
| Prompts | No | Get, List, Watch | 3 |
| Prompts.Answers | Yes | Create | 1 |
| Settings | No | Schema, Get, Set | 3 |
| Plugins.Settings | Yes | Get, Set | 2 |
| Storage.Sources | Yes | Get, Set | 2 |
| Secrets | No | List, Watch, Get, Set, Delete, Export, Import | 7 |
| Reports | No | Get, Create, Publish | 3 |
| Media.Captures | Yes | Create, Get, Watch, Stop | 4 |
| Aggregates | No | Get | 1 |

Total: 32 product client services (20 nested), 92 operations/gRPC methods,
92 requests and 92 responses. Current progress is also included in Executions.Get;
Progress.Set is absent from this client inventory.

The 132 rows reach 75 distinct product client destinations. Seventeen added
companions complete the 92-method client set: Cases.Watch, Cycles.Watch,
Documents.Watch, Executions.Watch, Executions.Progress.Get, Content.Artifacts.Watch,
Content.ExecutionFiles.Watch, Evidence.HealthEvents.Watch, Evidence.Notes.Get/Watch,
Evidence.Traceability.Watch, Media.Captures.Get/Watch/Stop, Plots.Settings.Get,
Prompts.Watch and Secrets.Watch. Workflow follow-ups in the mapping reasons use
these same canonical methods and do not increase the count.

### Complete Plugin-Facing Mapping

These 24 present rows cover 17 product context bindings, 2 library clock
operations and 5 dynamic lifecycle families. They are logical operations over the
library carrier, not additional client RPCs or protocol frames. Generic host
value delivery is library-owned; a product's resolver keeps settings/secrets
policy and storage. Dynamic identities do not multiply operation templates.

| Present Operation or Family | Locked Destination | Destination Owner | Preserved Behavior |
| --- | --- | --- | --- |
| `<template>.materialize` | TestCases.Materialize | Product | Renamed; pure materialization, request/response wrapper. |
| `<template>.initialize` | Executions.Initialize | Product | Renamed; acquire execution-local suite resources. |
| `Step.operation` | Steps.Execute | Product | Renamed family; frozen step identity and retry evidence. |
| `Criterion.operation` | Criteria.Evaluate | Product | Renamed family; captured-evidence judgment. |
| `<template>.cleanup` | Executions.Cleanup | Product | Renamed; required cleanup after initialization. |
| `host.execution.prompt` | Prompts.Create | Product | Renamed; wait for answer, preserve scope and cancellation. |
| `host.settings.read` | HostValues.Get | Library | Generic declared-value host operation; the product resolver supplies settings or secrets under caller scope and product policy. Product client settings/secrets services are not library stores. |
| `host.secrets.read` | HostValues.Get | Library | Generic declared-value host operation; the product resolver supplies settings or secrets under caller scope and product policy. Product client settings/secrets services are not library stores. |
| `harmos.clock.sleep` | Time.Sleep | Library | Kept meaning; normalized carriers, library-owned clock. |
| `harmos.clock.now` | Time.Get | Library | Renamed; library-owned clock. |
| `host.execution.record` | Evidence.Records.Append | Product | Renamed; ordered immutable data, durable receipt. |
| `host.execution.records` | Evidence.Records.List | Product | Merged; preserve count/last selection and bounded reads. |
| `host.execution.statistics` | Evidence.Statistics | Product | Merged; stream/field/step selectors and complete statistics. |
| `host.execution.artifact` | Content.Artifacts.Create | Product | First use library Storage.CreateFileProduction, AppendFileBytes, GetFileProduction, CreateFileWriter as needed, PublishFileProduction or DeleteFileProduction; Create registers the product artifact reference. |
| `host.execution.progress` | Executions.Progress.Set | Product | Host-only producer called by a plugin, using the whole progress value and Get revision; never client-facing Set. Get and Executions.Get return the current value; Watch observes it. |
| `host.execution.log` | Evidence.Logs.Append | Product | Renamed; ordered immutable log lines. |
| `host.execution.traceability` | Evidence.Traceability.Set | Product | Renamed; whole small writable value, Evidence.Traceability.Get revision and scope; large writable parts split into separately revisioned entry Get/Set sub-resources. |
| `host.execution.document` | Documents.AppendParts | Product | Renamed; ordered immutable document contributions. |
| `host.files.copy_files` | Files.Capture | Product | Renamed; freeze file copies, preserve originals. |
| `host.files.start_capture` | Files.Captures.Start | Product | Renamed; start capture lifecycle. |
| `host.files.stop_capture` | Files.Captures.Stop | Product | Renamed; finish capture at safe boundary. |
| `host.execution.environment` | Backend.Environment | Product | Renamed; derived environment read. |
| `host.execution.effect.prepare` | Effects.Create | Product | Renamed; durable inverse ticket, no hardware write. |
| `host.execution.effect.applied` | Effects.Set | Product | Renamed; whole writable application state and Effects.Get revision; preserve earliest inverse. |

Effects.Get is one added host/plugin-only companion, not a 25th present row.
The 18 distinct product templates additional to its client inventory are:

| Product Host/Plugin-Only Service | Additional Methods | Count |
| --- | --- | ---: |
| TestCases | Materialize | 1 |
| Executions | Initialize, Cleanup | 2 |
| Steps | Execute | 1 |
| Criteria | Evaluate | 1 |
| Prompts | Create | 1 |
| Evidence.Records | Append | 1 |
| Executions.Progress | Set | 1 |
| Evidence.Logs | Append | 1 |
| Evidence.Traceability | Set | 1 |
| Documents | AppendParts | 1 |
| Files | Capture | 1 |
| Files.Captures | Start, Stop | 2 |
| Backend | Environment | 1 |
| Effects | Create, Set, Get | 3 |

These add 7 logical services (TestCases, Steps, Criteria, Evidence.Logs, Files,
Files.Captures and Effects); Evidence.Logs and Files.Captures are nested. Seven
other namespaces in the table already exist in the product client inventory. The 18 operations consist of 10 methods in
those 7 added services and 8 methods in shared namespaces. They have 18 requests
and 18 responses; no client gRPC methods. Library HostValues.Get and Time.Get/Sleep
are counted only under the library. Shared product client operations are counted
once in the 110-template union.

### Nested Resource Proto Examples

These are separate illustrative files, not multiple packages in one file.
Domain values retain their owning package; field numbers are illustrative.

```proto
// proto/harmos/v1/plugins.proto
package harmos.v1;
message PluginsGetRequest { string name = 1; }
message PluginsGetResponse { PluginEntry entry = 1; bytes revision = 2; }
service Plugins {
  rpc Get(PluginsGetRequest) returns (PluginsGetResponse);
  // Create, List, Watch, Set, Restart and Delete have their own wrappers.
}
```

```proto
// proto/product/v1/plugins/settings.proto (product-owned)
package product.v1.plugins;
message SettingsGetRequest { string name = 1; }
message SettingsGetResponse { SettingsValue value = 1; bytes revision = 2; }
message SettingsSetRequest { SettingsValue value = 1; bytes expected_revision = 2; string request_id = 3; }
message SettingsSetResponse { SettingsValue value = 1; bytes revision = 2; harmos.v1.Receipt receipt = 3; }
service Settings {
  rpc Get(SettingsGetRequest) returns (SettingsGetResponse);
  rpc Set(SettingsSetRequest) returns (SettingsSetResponse);
}
```

```proto
// proto/product/v1/executions/progress.proto (client-facing product reads)
package product.v1.executions;
message ProgressGetRequest { string name = 1; }
message ProgressGetResponse { ProgressValue value = 1; bytes revision = 2; }
message ProgressWatchRequest { string name = 1; }
message ProgressWatchResponse { ProgressValue value = 1; bytes revision = 2; }
service Progress {
  rpc Get(ProgressGetRequest) returns (ProgressGetResponse);
  rpc Watch(ProgressWatchRequest) returns (stream ProgressWatchResponse);
}
// No client Set RPC. The plugin calls a separately declared host operation.
// product.v1.ExecutionsGetResponse also contains the current ProgressValue.
```

```proto
// proto/harmos/v1/access/listener.proto
package harmos.v1.access;
message ListenerGetRequest {}
message ListenerGetResponse { ListenerValue value = 1; bytes revision = 2; }
message ListenerSetRequest { ListenerValue value = 1; bytes expected_revision = 2; string request_id = 3; }
message ListenerSetResponse { ListenerValue value = 1; bytes revision = 2; harmos.v1.Receipt receipt = 3; }
service Listener {
  rpc Get(ListenerGetRequest) returns (ListenerGetResponse);
  rpc Set(ListenerSetRequest) returns (ListenerSetResponse);
}
```

```proto
// proto/harmos/v1/status/logs.proto
package harmos.v1.status;
message LogsListRequest { harmos.v1.PageRequest page = 1; harmos.v1.ReadCut at = 2; }
message LogsListResponse { repeated LogLine lines = 1; harmos.v1.ReadCut resolved_at = 2; string next_page_token = 3; }
message LogsWatchRequest { harmos.v1.ReadCut at = 1; }
message LogsWatchResponse { LogEvent event = 1; harmos.v1.ReadCut resolved_at = 2; }
service Logs {
  rpc List(LogsListRequest) returns (LogsListResponse);
  rpc Watch(LogsWatchRequest) returns (stream LogsWatchResponse);
}
```

Plots.Settings and Status.TraceEvents follow the same per-service file,
subpackage and wrapper rule. A host-only operation wrapper is not a client RPC:
its descriptor/declaration records exposure separately from its logical name.

### Complete Native Provider Mapping

All 66 present hooks below have Library-owned destinations. They are not additional
RPCs or resource services. The two
provider traits cover multiple resources; their semantic names retain those
resource nouns, without repeating the provider's own Storage name. Locked names below are semantic
names; implementations retain normal language casing. Private provider hooks
such as explicit flush remain private mechanisms, not public grammar exceptions.
Every unchanged guarantee remains required even when a hook is renamed or merged.

| StorageBackend Hook | Locked Name | Disposition and Preserved Responsibility |
| --- | --- | --- |
| history | GetHistory | Renamed; store identity/epoch. |
| open | CreateSession | Renamed; open durable provider. |
| cursor_saved | GetSavedPosition | Renamed; passive saved frontier. |
| close | DeleteSession | Renamed; settle and release provider. |
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
| capabilities | Capabilities | Renamed; incorporate support flags below. |
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
| supports_file_sessions | Capabilities | Merged; no separate boolean owner. |
| supports_file_deletion | Capabilities | Merged; explicit deletion support. |
| supports_mutation_receipts | Capabilities | Merged; explicit durable replay support. |
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
open under SDKs; four-language managed-execution parity is not a landed fact or an approved
new requirement. Descriptor-derived client-root naming in all four languages is
separately locked.

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

Product-only host-operation gaps remain product work. Generic plugin, access,
transfer and status gaps are now library work under the manager inventory above.
The product desktop moves once, after all services have their final names and the
exact library revision is available; it consumes composed services directly.

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

The tasks inventory fixes the dependency order. First shared primitives and final
descriptor/service names; then mechanically generated wrappers and hierarchical
roots, migrating each affected producer and consumer in one compiling slice.
Integrate existing plugin-manager work rather than duplicate it. Add Access under
GrpcManager, Status and product-fact registration under ServiceManager, and the
generic host-value mechanism under OperationManager. Reuse Storage sessions and
the existing StreamManager. Preserve stored encodings; update conformance,
examples and affected guidance with the corresponding code.

Under later authorization publish/pin one exact library revision. The product
defines its storage layer and policy, composes the library services directly,
splits execution/prompt/report/retained-recording orchestration and migrates its
SDKs and CLI. Delete its duplicate plugin connection, dispatch, streams, process
launch, entry management, listener/pairing, upload handling and host-status code
in the same compiling adoption slices. Product-only operations call their manager
or storage through one operation file per service method.

Once every service has its final name, migrate the product desktop and regenerate
its root exactly once. Do not move it to an intermediate host or grammar. Remove
all remaining superseded code only after consumers migrate and Nightly large-file
throughput is measured. No compatibility aliases, second long-lived authorities,
implicit data migration or stored-data reset. Rollback uses a matched prior
host/SDK revision. Final validation remains separate from source closure.
