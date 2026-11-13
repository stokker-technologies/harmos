# Operation Grammar Design

Status: locked by the owner on 2026-11-12; paging amended by the owner on 2026-11-13
Boundary, managers and nested resources locked by the owner on 2026-11-12
Naming rule amended by the owner on 2026-11-12: bare verbs on the service's resource
Separation by audience locked by the owner on 2026-11-12
Three visibility levels locked by the owner on 2026-11-12

Locked contracts remain authoritative; implementation sections below describe landed source.
Only the explicitly listed open questions await decisions when their work is reached.

## Owner Confirmations, 2026-11-13

- Owner decision: the host clock service keeps the name `Time`, with `Time.Get`
  and `Time.Sleep`; no clock-service rename is authorized.
- Owner decision: library release waits until the consumer product has stabilized
  on this branch. Task 5.1 stays open; preparing this candidate does not authorize
  publication or a consumer pin change.

## Context

See proposal.md for motivation and decision status. Both supplied research
reports were read in full. Current declarations, provider traits, protocol rules,
SDKs and documentation were checked where locally available. Verification paths below
are relative to this repository; sibling facts are described generically.

## Goals and Non-Goals

Make public operations predictable without flattening storage guarantees. Keep
consumer policy with consumers, and retain native storage optimization and
recovery responsibilities. Do not add a second scheduler, silently migrate stored
encodings or promise arbitrary historical presentation. Keep the library protocol,
SDKs and conformance checks self-sufficient without product vocabulary.

## Wire Operation Identity Admission

Wire identities are either nonempty dot-separated ASCII alphanumeric/underscore segments
(numeric segments included), or `package.Service/Method`, whose package segments, service
and method are protobuf identifiers (`[A-Za-z_][A-Za-z0-9_]*`), with exactly one slash and
a nonempty package. A versioned `plugin.vN` or `context.vN` package segment pair selects
that face (including nested packages); other packages select the client face. Multiple
audience markers are refused. This separates transport names from native dotted names
while preserving template-local numeric bindings and protobuf method identities.

`crates/harmos-protocol/src/schema.rs` owns validation and face classification; native
`OperationDefinition::validate` retains its existing local identity policy. Plugin catalogs
use the same validator with plugin-face and reserved-library admission checks. Requirements
may reference another face, but dispatch refuses client-only methods to plugins and context
or instance methods to clients; Public catalog operations remain host-mediated exceptions
carried by Instance.Invoke. Application-owned context declarations need no client binding.
Descriptors, fingerprints, explicit visibility and template-local provider selection remain
independent admission checks.

Provider assignments validate the native owner separately and their local wire identity
with the same parser; assignments stay frozen and unique by local identity.

The audience/visibility integration added retained protobuf metadata to admitted plugin
entries. That correctly enabled client dispatch and policy checks, but reused native dotted
validation for their wire IDs. Restore wire validation without removing the retained metadata.

## Shared Grammar and Vocabulary

The following shared text uses the supplied decisions and both reports' Shared
Vocabulary sections. Application-specific meanings belong to their owners.

- **Audience Faces (Owner, 2026-11-12):** every wire service belongs to exactly
  one face, visible in its package. The plugin protocol is served by a running
  plugin and called only by the host: `harmos.plugin.v1.Instance`, with Get
  (identity and catalog), Invoke, Samples and Stop. The context is served by the
  host to a plugin handling an invocation, through its context object as child
  calls: `harmos.context.v1`. The client API is served by the host to applications,
  command-line tools and scripts: `harmos.v1`. Clients cannot call context or
  instance methods; plugins cannot call client-only methods. The host opens every
  instance call; plugins and clients never connect directly to an instance.
  Product declarations follow `<product>.plugin.v1`, `<product>.context.v1` and
  `<product>.v1` for the same audiences. Host-requested plugin lifecycle operations are
  Protected catalog operations carried by Instance.Invoke, never client services;
  Private implementation operations stay local and have no face declaration. No
  product resource or operation is declared by this library change.
- **Face-Local Contracts (Owner):** a capability needed on two faces has one
  declaration per face, with its own request, response, full operation ID and
  fingerprint, even when both declarations share a handler. Each face evolves
  and is authorized independently. Shared domain payloads/primitives are allowed;
  importing another face's method wrappers as one's own is not. Context
  declarations are invoked as child calls, not opened as independent client RPCs.
- **Visibility (Owner, Three Levels Locked 2026-11-12):** faces do not replace
  visibility. A face says who serves a gRPC service and who may call it on the
  wire. Every SDK operation explicitly chooses Private, Protected or Public,
  without a default. Private is plugin-internal: only the plugin itself invokes
  it inside its process. It is absent from the catalog, never travels on any
  wire (including the host-to-plugin carrier), cannot bind a gRPC method and is
  invisible to the host. The SDK dispatches it locally with the same policy,
  retries, deadlines and cancellation handling as any operation. Protected is
  catalogued and callable by the plugin and its host over that carrier only;
  other plugins and clients are refused, and no client-facing gRPC binding is
  allowed. Public is exposed: other plugins with a declared requirement and
  clients invoke it through the host; a client-facing gRPC binding is permitted.
  Admission rejects unspecified visibility and any Private catalog entry or
  Private/Protected client-facing binding. The same levels apply to built-ins;
  their decided mapping is below. The owner's purposes are: "a private operation
  never appears on the wire, is consumed only internally by the plugin and does
  not allow association to a grpc endpoint"; "a plugin should be able to call
  another operation in another plugin if exposed".
  A plugin can invoke another plugin's Public operation: plugin, host, plugin.
  The calling plugin declares a requirement and makes a child call through its
  context. The host resolves it in its one operation registry, checks visibility,
  the declared fingerprint and that a provider is admitted, then invokes the
  providing instance with the caller's unchanged inherited scope. The result or
  structured failure returns through the host; cancellation reaches the child.
  A client invokes a Public plugin-served operation through the generic
  `harmos.v1.Invocations.Invoke` path, with the same host checks and routing.
  Protected operations refuse other plugins and clients with an error naming the
  attempted action; Private operations have no host gRPC method. Host mediation
  never broadens visibility. A host application's operations that only it may
  request of a plugin are declared Protected, never Private or Public.
- **One SDK per Language (amended by the owner on 2026-11-13):** Python ships
  `harmos-sdk` (import `harmos`), Rust ships `harmos`, C# ships NuGet `Harmos`,
  and C++ ships CMake `HarmosCpp` with target `Harmos::Cpp`. Each contains plugin
  authoring, its client root and the host's protocol needs. The separate Rust
  runtime library remains the host composition owner, not a second SDK.
  There is no audience wheel, Cargo feature, MSBuild/CMake switch or generated
  service-stub guard. Context methods remain child-carried rather than standalone
  services. Host admission and SDK-local Private dispatch enforce the three faces
  and three visibility levels at runtime; possession of a stub grants no authority.
- **Names (Owner):** a service names its resource with a plural noun, or a
  singular noun for one thing, such as Instance. Methods acting on that resource
  are bare verbs. A part read in exactly one way keeps its noun on its parent:
  Jobs.Steps is a bounded page.
  A part with more than one read/change mode is a nested resource with its own
  bare verbs: Jobs.Progress.Get/Watch and Documents.Settings.Get/Set. This replaces
  the earlier verb-plus-part rule and flat compound services. Mixed-resource Storage retains distinguishing resource nouns
  (ListRows/WatchRows, GetObject/SetObject); these are independent storage resources,
  not differently named parts of one resource. It is not split merely for brevity.
  Always return a response wrapper, including empty replies. Within each package,
  messages are `<Service><Method>Request/Response`, such as SettingsGetRequest in
  application.v1.plugins. Package qualification disambiguates equal message names.
  Operation IDs derive mechanically from the complete service namespace and method.
- **Nested Wire Layout (Owner):** a nested service lives in package
  `<face package>.<lowercase parent>`, where the face package is
  `<owner>.plugin.v1`, `<owner>.context.v1` or `<owner>.v1`, with the part as service name. One proto file per service, in a
  folder per parent: application/v1/plugins/settings.proto declares
  application.v1.plugins.Settings; its Get method's full name is
  application.v1.plugins.Settings/Get. Deeper parents extend the package path the same
  way. Namespace-only parents need no empty service. A nested resource may have
  a different owner: the library declares harmos.v1.Plugins while a product
  declares application.v1.plugins.Settings on the same host endpoint. This is intended.
- **Client Roots (Owner):** each of Rust, Python, C# and C++ exposes the same
  hierarchy from service packages/names, with its normal language casing:

  ```python
  client.plugins.settings.get(request)
  client.jobs.progress.watch(request)
  client.jobs.start(request)
  ```

  A small generator in each SDK build consumes a common table produced from the
  descriptor set: owner namespace, face, version, parent path, service, method, wrappers
  and cardinality. Strip the selected face package prefix, append lowercase parent/service
  segments, and bind generated transport clients at those paths. Compose descriptor
  tables for library and product services into one root, so library Plugins and
  product plugins.Settings share the plugins node. Reject actual leaf collisions;
  preserve package-qualified message identities. No hand-written per-operation
  naming or forwarding layer. This four-language naming commitment does not settle
  generic managed-client invocation coverage or plugin semantic parity.

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
  each with its own Get and Set. Documents.Set carries the whole small writable
  value returned by Documents.Get; only that identified document changes.
- **Domain verbs (Owner):** use a domain verb only where a standard verb would
  hide a side effect, and state its one meaning. Start activates; Stop finishes at
  a safe boundary; Cancel interrupts; Restart replaces an activation; Publish commits
  prepared visibility; Import admits an external representation; Export produces an external representation; Invoke
  calls a declared operation; Advance materializes a projection frontier; Capture
  freezes external data. Storage.PublishFileProduction validates and publishes an upload;
  Storage.DeleteFileProduction abandons its unpublished session. Plugin lifecycle verbs keep
  their distinct meanings. The clock wait keeps the name Sleep.
- **Operation and invocation (Owner):** an operation is a declaration, an
  invocation is one call of it, and an attempt is one retry attempt.
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
  lease or content digest. A scalar sequence alone cannot represent a composite cut.
- **Reads (Owner):** one read path per kind of data; optional position `at`;
  absent resolves to latest; the reply states `resolved_at`; paging and watches
  follow the same rule. Accept `at` only as a position a previous reply returned
  for consistency, or a product-defined terminal boundary. Arbitrary earlier
  positions are not offered. The product decides what latest and its terminal
  boundary mean; the library does not define product lifecycles.
- **Latest and durability (amended by the owner on 2026-11-13):** latest resolves
  once at read/watch opening to the latest durable position without waiting for
  accepted data. A caller requiring its own write passes its receipt's durable
  position as `at`. The owner accepts measured p95 lag: small about 99 ms; large
  median 391 ms (178–570); mixed median 346 ms (173–493). No accepted-data snapshot
  is built. Nightly reports p50, p95 and maximum per load without a failing threshold.
- **SDK ownership (Owner):** language SDKs stay in the library because other
  products will also host plugins on it; product SDKs keep building on them.
  Protocol, SDKs and conformance remain self-sufficient without product vocabulary.
- **Paging (amended by the owner on 2026-11-13):** only unbounded data rows are paged.
  Bounded installed/configured sets return in one response without a page request, token
  or byte bound; the carrier message size limit is their only response guard, and
  exceeding it fails with an error naming the action. Unbounded reads use `page_size`, `max_bytes`, `page_token`, `next_page_token`,
  `resolved_at`; tokens bind selection, ordering, cut and schema/projection revision.
  Stable ordering has an identifier tie-breaker and exclusive continuation. Bound
  rows and bytes; work bounds remain separate. Expired or mismatched tokens fail
  explicitly, never restart silently at latest.
- **Watching (Owner):** use optional `at` and return `resolved_at` under the read
  resolution rule. The Durable Row Watch Contract below settles row opening,
  exclusive committed events, filtered progress and resynchronization. External
  file/directory and runtime watches retain their own distinct guarantees.
- **Time (Owner):** protobuf Timestamp for instants and Duration for elapsed
  budgets; host observation time and optional source time remain distinct. Acceptance
  and saved time are separate. Source-time windows are half-open filters, separate
  from ingestion positions.
- **Failures (Owner):** one Failure vocabulary: classification, stable code,
  message, action, operation, plugin, cause and safe details; preserve causes and
  map consistently to canonical gRPC status/details. Diagnostics remain separate.
  Missing data, gaps and unavailable positions cannot become empty success.
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

### List Inventory (Amended by the Owner on 2026-11-13)

| List | Disposition and Reason |
| --- | --- |
| CatalogEntries.List (client and context; operations, schemas, streams and objects) | Unpaged catalog metadata: declarations and installed schema/identity definitions form the admitted catalog, rather than stored record/sample/revision data. |
| Plugins.List | Unpaged: entry count is bounded by the configured registry capacity. |
| Access.Pairings.List | Unpaged: credential count is bounded by the configured pairing capacity. |
| Native ListSchemas | Complete catalog enumeration uses internal bounded scans; schemas are definitions, not data rows. No external continuation is exposed by CatalogEntries. |
| Storage.ListRows | Paged and position-resolved: records, samples and object revisions grow with ingestion. |
| Storage.ListColumnRows | Paged and position-resolved: physical stored rows grow with ingestion. |
| Storage.ListIndexEntries | Paged and position-resolved: index entries grow with stored rows. |
| Storage.ListProjectionRows | Paged and position-resolved: materialized data rows grow with ingestion. |
| Storage.ListDirectoryEntries | Paged and position-resolved: files/directories can grow without bound. |
| Status.Logs.List and Status.TraceEvents.List | Paged and position-resolved: events accumulate indefinitely even when retention bounds the current window. |
| Native ListObjectRevisions/ListRecords/ListSamples/ListFiles/ListIdentities/ListRecordViews/ListRecoveryEntries/ListMutationReceipts/ListFileProductionCheckpoints | Paged internally: persisted data, identities, recovery entries and session/receipt histories grow with use. |

Installed catalog metadata and configured management sets have no application response
byte bound or page token. They retain authorization and existing read-cut semantics;
carrier overflow refuses the complete action instead of truncating success.

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
| Plugin manager | Plugins: managed/remote entries; Create, Get, List, Watch, Set, Restart, Delete; verified bundles, persistence, state and failures; Instance carrier placement | Generic plugin-manager work is already being implemented; integrate it. The current guest adapter has PluginManager in guest/host/plugin/manager.rs, not proof the complete entry service has landed. |
| gRPC manager | Access and Access.Listener: listener Get/Set, pairing creation, certificate regeneration/key rotation; listener shutdown | GrpcManager in grpc/manager.rs exists. Access service contracts are added by this decision. |
| Storage manager | Storage, including the six production/session actions and ranged GetFileBytes shared by clients/plugins | StorageManager in storage/manager.rs exists. A product supplies storage definitions/providers, not a second upload engine. |
| Service manager | Status, Status.Logs and Status.TraceEvents; host identity, health, availability, logs, traces and diagnostics archive | ServiceManager in services/mod.rs exists. Add the status contracts and narrow product-fact registration here. |
| Operation manager | CatalogEntries, Invocations and context-only HostValues; operation registration, dispatch, invocation lifecycle and protocol callbacks | OperationManager in operations/manager/mod.rs exists; its inner Manager is not a second public owner. |
| Stream manager | Samples and Instance.Samples publication/demand coordination | StreamManager in streams/manager.rs exists. Plugin protocol serving remains library-owned. |
| Service manager / host clock | Time.Get and Time.Sleep, backed by the authoritative runtime clock | Reuse existing service lifecycle and clock ownership; no second clock manager is required. |
| State manager | State lifecycle, replay and recovery; no new public grammar service | StateManager in state/manager.rs exists. |
| Record manager | Record registration and lifecycle; persistence through Storage | RecordManager in records/manager.rs exists. |

Verification paths in this table are relative to crates/harmos-runtime/src/.
Existing types identify orchestration homes, not claims that the new
client-facing services are implemented.

Status describes the host process. A narrow ServiceManager registration lets a
product contribute its version/identity facts and component health/availability
through typed providers with host-owned lifetime and failure reporting. It does
not let a product replace generic collection or create a competing status service.
Diagnostics.Get returns bounded unary archive ranges through a narrow content-source
boundary that joins the shared storage range mechanism.

Settings and secrets are too opinionated for a generic library. A product owns
their stores, schemas, user-interface declarations, archives and operating-system
credential-store use. The library owns only a settings-declaration section of the
plugin catalog and context-only HostValues.Get: a plugin requests a declared value,
the host application's registered resolver supplies it under product policy.
No library settings/secret store, UI schema or client-facing settings service is
introduced. This closes the per-plugin settings/secrets ownership question.

### Library Interpretation of Shared Decisions

The shared vocabulary uses generic resources. A host application decides what
latest and a terminal read boundary mean for its own objects; the library does
not define application lifecycles or equate a read boundary with a content digest.

**Owner, 2026-11-12:** Cancel applies to running work only; queued control
refuses Cancel with an error naming the action. Stop finishes at a safe boundary
and remains distinct. Renaming changes one identified resource, not its sibling
attempts; the library preserves the caller-selected mutation scope.

**Owner:** Set validates and atomically replaces the whole writable value under
its expected revision. Saved-data watches must not label accepted data durable;
the Durable Row Watch Contract below settles row events and opening; external
file/directory and runtime sources retain their own contracts.

### Current Verification and Corrections

| Finding | Current Tree Verification and Interpretation |
| --- | --- |
| 41 built-ins | `crates/harmos-operations/src/{host,lifecycle,storage,streams,clock}/`: 1 catalog, 3 invocation controls, 34 storage, 1 publication, 2 clock operations. Clock is registered separately from `builtins_catalog()`. |
| 30 projected bindings | `const GRPC` declarations in those directories: 24 storage, 4 host/control and 2 clock. |
| 4 plugin carriers | `proto/harmos/v1/plugin.proto` and runtime `plugin/grpc/{connection,invoke,publish}.rs`; all four SDKs implement them. |
| 11 static Storage methods | `proto/harmos/v1/storage.proto:service Storage` is a subset of 34 declarations. Rust `sdk/rust/src/storage_contracts.rs` also binds 11 requests. |
| Automatic twins | `crates/harmos-runtime/src/grpc/projection.rs` emits `Run{method}` and managed envelopes unconditionally. |
| Durable snapshots | `crates/harmos-storage/src/snapshot.rs` and `docs/storage.md` reject positions beyond saved progress; follows also join accepted data today. That is not an existing durable-watch contract. |
| Flush cadence and commits | `crates/harmos-storage/src/config.rs` defines 100 ms, a 1 MB batch target and a 16 MB cap; `worker.rs` schedules flushes; `sqlite/mod.rs` sets WAL and `synchronous=FULL`. Each batch commits synchronously under `backend.rs`; flush duration adds to the cadence and grows with bytes written. The three-load benchmark below measures durable lag. |
| Native inventory | `StorageBackend` in `backend.rs` has 58 hooks; `StateBackend` in `state/mod.rs` has 8. Other methods in that module are implementations, not extra provider hooks. |

The library report's present-operation totals agree with this tree. Its suggested
WaitTime is superseded by Sleep. Its bare-resource replies are superseded by response wrappers; whole-value
revision-guarded Set is locked. Its barrier-before-
latest recommendation is
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

**Amended by the owner on 2026-11-13:** latest resolves to the latest durable
position without waiting. Durable lag is accepted as measured: small about 99 ms,
large median 391 ms (178–570), mixed median 346 ms (173–493) at p95. There is no
failing lag threshold and no accepted-data snapshot is built. Nightly continues
reporting p50, p95 and maximum for each load as measurements.

The 100 ms cadence and existing 1 MiB early-flush trigger are unchanged. Each
commit retains the existing atomic admission ceiling (at most 16 MiB within the
consumer's pending-write budget) and synchronous durability. Logical input and
larger internal state captures split before exceeding that ceiling. The earlier
1 MiB per-commit split was measured, not assumed beneficial: it multiplied the
large workload's FULL commits from 128 to 1,664. The current implementation reuses
checked admission sizes, isolates large admissions from preceding small prefixes,
and publishes each acknowledged committed piece immediately. SQLite independently
compacts or splits physical column chunks inside the same atomic transaction;
its configured row/byte bounds and stored encodings are preserved.

`integration/durable_lag` runs five repetitions each of `small_high_rate`,
`multi_megabyte` and `mixed`, retaining per-load p50/p95/maximum, spread, backlog,
admission wait, compiler-activity samples and stage counters. The counters separate
waiting for the first cadence/size trigger, ordered queue wait, batch construction,
provider writing, synchronous commit and saved-frontier publication. They measure
elapsed work, including scheduling/I/O contention, and never drive admission or
assert real time in tests. Reads select an issued saved position without requesting
a flush or a newer durability barrier. Persistence failures remain visible.

The small-pieces/early-flush experiment has therefore been performed. Repeated
results below record the accepted observed floor. Accepted-data
snapshots remain unbuilt: they would require a new retention/read-image mechanism,
could expose accepted data independently of writer I/O, and would not make a FULL
commit finish sooner or justify acknowledging durability before it.

| Alternative | Cost and Disposition |
| --- | --- |
| Latest durable without waiting | Owner amendment of 2026-11-13: coherent saved data without a new persistence wait; measured lag is accepted, and the three-load Nightly benchmark continues reporting without a failing threshold. |
| Barrier on every latest read | Not chosen: latency depends on queued data and a synchronous commit; a passive read requests persistence. An explicit durability wait remains available to writers. |
| Snapshot of accepted data | Owner decision after the measured floor: not built. Requires retention/read images spanning accepted data and mutable state; existing saved snapshots cannot provide it. It can decouple read visibility from persistence, not accelerate durable acknowledgement. |

A write receipt containing only `accepted_at` cannot be used as proof of
durability. Obtain `durable_at` from that write's completion/status before a read
requiring read-your-write visibility. If an issued cut has expired, fail explicitly;
never fall back to a newer cut. Validate issued-position provenance and retention
without treating knowledge of a sequence as authorization.

### Representative Public Shapes

These excerpts express the locked contract, not compilable replacement files or field-number
compatibility commitments. Domain payloads named below already have owners; the
package layout and file moves precede the complete descriptor inventory. Optional message
fields use protobuf message presence. The four representative resources are rows, objects,
invocations and file productions. Watch event unions illustrate candidate details
for the still-open exact Watch semantics; the naming and read-resolution rules
are locked.

```proto
// Client-face primitive and time excerpt; shared types retain their owner.
package harmos.v1;
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
package harmos.v1;
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

Instance.Samples observes publication demand; it has no historical List on the
plugin carrier. Retained sample reads belong to host storage. Do not manufacture
catalog watches without an actual change feed, and retain device-driven streams
alongside host-clock interval streams.

```proto
package harmos.v1;
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
message InvocationsInvokeRequest {
  string operation = 1;
  bytes fingerprint = 2;
  bytes payload = 3;
}
message InvocationsInvokeResponse { bytes payload = 1; }
// Caller scope is bound invocation context, not an authority-bearing payload.
// Failures retain the shared structured gRPC status/details mapping.
service Invocations {
  rpc Invoke(InvocationsInvokeRequest) returns (InvocationsInvokeResponse);
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
package harmos.v1;
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
package harmos.v1;
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
As amended by the owner on 2026-11-13, delete streaming transfers after the measured
comparison and retain unary Nightly `integration/file_transfer` measurements, reporting bytes,
chunk/range configuration, carrier and throughput. This is a benchmark, never a
test assertion or a new hard throughput threshold.

### Complete Public Operation Mapping

Every current logical operation appears below; a shared capability maps to a
separate declaration on each required face. Native-only hooks stay outside the
wire faces. The Face and Package column qualifies every locked destination.
The removed streaming-upload convenience maps to the shared session entry point;
its subsequent workflow calls are not additional destinations. Present types use the current
request and final reply; current stream item types are noted in the reason. Locked
request/reply names always follow the method-wrapper rule. All target names and
dispositions are Owner decisions, including retained names whose payloads change.

Verification key: paths below are under `crates/harmos-runtime/tests/storage/` for
storage tests, under `crates/harmos-operations/src/` for declarations, and under
`crates/harmos-runtime/src/plugin/grpc/` for plugin-host callers. Named test sources
are invocation verification, not claims that tests were run in this authoring task.

| Present Operation | Present Request → Final Reply | Locked Operation | Face and Package | Disposition and Reason / Caller Verification |
| --- | --- | --- | --- | --- |
| carrier Describe | DescribeRequest → Description | Instance.Get | Plugin protocol: harmos.plugin.v1 | Renamed; connection.rs and SDK servers use identity/catalog admission. |
| carrier Invoke | HostFrame → PluginFrame | Instance.Invoke | Plugin protocol: harmos.plugin.v1 | Renamed service; invoke.rs; duplex control carrier still conveys one unary operation attempt. |
| carrier Publish | PublishFrame → Samples | Instance.Samples | Plugin protocol: harmos.plugin.v1 | Renamed; publish.rs; host-clock ticks and device publication remain distinct. |
| carrier Stop | StopRequest → StopResponse | Instance.Stop | Plugin protocol: harmos.plugin.v1 | Renamed service; connection.rs shutdown invokes cleanup. |
| host.catalog | Catalog → CatalogPage | CatalogEntries.List | Context: harmos.context.v1; client: harmos.v1 | Renamed; sdk/rust/src/client.rs lists the complete admitted catalog. |
| host.operations.status | Status → InstanceState | Invocations.Get | Context: harmos.context.v1; client: harmos.v1 | Renamed; tests/operations/host.rs invokes owned status. |
| host.operations.cancel | Cancel → InstanceState | Invocations.Cancel | Context: harmos.context.v1; client: harmos.v1 | Renamed; same host test; cancellation is an action. |
| host.operations.watch | Watch → InstanceState | Invocations.Watch | Context: harmos.context.v1; client: harmos.v1 | Renamed; same host test; retain status events. |
| harmos.clock.now | Now → Time | Time.Get | Context: harmos.context.v1; client: harmos.v1 | Renamed; four SDK contexts call the host clock. |
| harmos.clock.sleep | Sleep → TimeSleepResponse | Time.Sleep | Context: harmos.context.v1; client: harmos.v1 | Kept; four SDK contexts; locked clock action, not WaitTime. |
| host.publish | Publish → Published | Samples.Append | Context: harmos.context.v1 | Renamed; streams/publish.rs and sdk/rust/src/wasm/context.rs publish immutable samples. |
| storage.status | StorageStatus → StorageInfo | Storage.Info | Native only: no wire package | Renamed; generated_status.rs. |
| storage.capabilities | Capabilities → StorageCapabilities | Storage.Capabilities | Context: harmos.context.v1; client: harmos.v1 | Renamed; generated_native_journey.rs. |
| storage.open_snapshot | OpenSnapshot → SavedSnapshot | Storage.CreateSnapshot | Context: harmos.context.v1; client: harmos.v1 | Renamed; same journey; retained backend read lease, not a second read family. |
| storage.release_snapshot | ReleaseSnapshot → Empty | Storage.DeleteSnapshot | Context: harmos.context.v1; client: harmos.v1 | Renamed; same journey; release the lease. |
| storage.scan_arrow | ScanArrow → ArrowPage | Storage.ListColumnRows | Context: harmos.context.v1; client: harmos.v1 | Renamed; same journey; preserve bounded physical work. |
| storage.scan_index | ScanIndex → IndexPage | Storage.ListIndexEntries | Context: harmos.context.v1; client: harmos.v1 | Renamed; same journey; ordered seek semantics remain. |
| storage.register_index | RegisterIndex → DefinitionRevision | Storage.SetIndexDefinition | Context: harmos.context.v1; client: harmos.v1 | Renamed; same journey; whole definition and expected revision from GetIndexDefinition; rebuild preconditions remain. |
| storage.register_projection | RegisterProjection → DefinitionRevision | Storage.SetProjectionDefinition | Context: harmos.context.v1; client: harmos.v1 | Renamed; same journey; whole definition and expected revision from GetProjectionDefinition. |
| storage.advance_projection | AdvanceProjection → DefinitionRevision | Storage.AdvanceProjection | Context: harmos.context.v1; client: harmos.v1 | Kept; same journey; side effect is checked materialization. |
| storage.read_projection | ReadProjection → ProjectionPage | Storage.ListProjectionRows | Context: harmos.context.v1; client: harmos.v1 | Renamed; same journey; exact revision/frontier availability. |
| storage.inspect_projection | InspectProjection → ProjectionState | Storage.GetProjectionStatus | Context: harmos.context.v1; client: harmos.v1 | Renamed; same journey; derived status is not writable. |
| storage.compare_exchange_object | CompareExchangeObject → CompareExchangeOutcome | Storage.SetObject | Context: harmos.context.v1; client: harmos.v1 | Renamed; same journey; whole writable object and expected revision from GetObject; atomic conditional outcome. |
| storage.query | Query → QueryCompletion | Storage.ListRows | Context: harmos.context.v1; client: harmos.v1 | Renamed; generated_query_batches.rs and generated_query_projection.rs; bounded unary pages replace QueryOutput streaming. |
| storage.follow_records | FollowRecords → StorageCursor | Storage.WatchRows, records selector | Context: harmos.context.v1; client: harmos.v1 | Merged; generated_record_follow.rs; shared boundary/change union. |
| storage.follow_stream | FollowStream → StorageCursor | Storage.WatchRows, samples selector | Context: harmos.context.v1; client: harmos.v1 | Merged; generated_stream_follow.rs; add consistent initial durable boundary. |
| storage.follow_objects | FollowObjects → StorageCursor | Storage.WatchRows, object-revision selector | Context: harmos.context.v1; client: harmos.v1 | Merged; generated_object_follow.rs; preserve revision data. |
| storage.write_schema | WriteSchema → StorageCursor | Storage.SetSchema | Native only: no wire package | Renamed; operation_schemas.rs; whole small writable schema and expected revision from GetSchema; immutable revision rules remain. |
| storage.write_records | WriteRecords → WriteCompletion | Storage.AppendRecords | Context: harmos.context.v1; client: harmos.v1 | Renamed; generated_record_exchange.rs; bounded unary batch replaces DataBatch/WriteReceipt exchange. |
| storage.write_streams | WriteStreams → WriteCompletion | Storage.AppendSamples | Context: harmos.context.v1; client: harmos.v1 | Renamed; generated_exchange_cancellation.rs; preserve admitted outcomes with receipts. |
| storage.write_objects | WriteObjects → WriteCompletion | Storage.AppendObjectRevisions | Context: harmos.context.v1; client: harmos.v1 | Renamed; generated_object_exchange.rs; bulk ingestion does not replace atomic SetObject. |
| storage.write_file | WriteFile → FileReference | Storage.CreateFileProduction | Context: harmos.context.v1; client: harmos.v1 | Removed; unary_transfers.rs; maps to the session entry point, then Storage.AppendFileBytes and Storage.PublishFileProduction replace the streaming convenience workflow. |
| storage.create_file | CreateFile → FileProducer | Storage.CreateFileProduction | Context: harmos.context.v1; client: harmos.v1 | Renamed; generated_file_mutations.rs; create the upload/production session. |
| storage.append_file | AppendFile → FileProducer | Storage.AppendFileBytes | Context: harmos.context.v1; client: harmos.v1 | Renamed; same mutations; bounded immutable chunks. |
| storage.resume_file | ResumeFile → FileProducer | Storage.CreateFileWriter | Context: harmos.context.v1; client: harmos.v1 | Renamed; same mutations; fence the previous writer. |
| storage.finish_file | FinishFile → FileReference | Storage.PublishFileProduction | Context: harmos.context.v1; client: harmos.v1 | Renamed; same mutations; durable final reference. |
| storage.abandon_file | AbandonFile → FileProgress | Storage.DeleteFileProduction | Context: harmos.context.v1; client: harmos.v1 | Renamed; same mutations; Delete of the unpublished session abandons it. |
| storage.delete_file | DeleteFile → StagedFile | Storage.DeleteFile | Native only: no wire package | Kept; same mutations; retained-reader decision remains open. |
| storage.read_file | ReadFile → FileCompletion | Storage.GetFileBytes | Context: harmos.context.v1; client: harmos.v1 | Replaced; unary_transfers.rs; streaming download removed in favor of bounded unary ranges with offset, maximum bytes, total length and digest. |
| storage.hash_file | HashFile → FileHash | Storage.GetFileHash | Context: harmos.context.v1; client: harmos.v1 | Renamed; guest SDK and the neutral transfer example call it. |
| storage.capture_file | CaptureFile → FileReference | Storage.CaptureFile | Context: harmos.context.v1; client: harmos.v1 | Kept; generated_capture.rs; independent captured bytes. |
| storage.follow_file | FollowFile → FileCompletion | Storage.WatchFileBytes | Context: harmos.context.v1; client: harmos.v1 | Renamed; generated_capture.rs; FileEvent preserves replacement/truncation. |
| storage.follow_production | FollowProduction → FileProgress | Storage.WatchFileProduction | Context: harmos.context.v1; client: harmos.v1 | Renamed; generated_reconnect.rs and generated_future_offset.rs; writer fencing and retained chunks remain. |
| storage.list_directory | ListDirectory → DirectoryCompletion | Storage.ListDirectoryEntries | Context: harmos.context.v1; client: harmos.v1 | Renamed; generated_directory.rs; bounded DirectoryEntry selection. |
| storage.watch_directory | WatchDirectory → DirectoryCompletion | Storage.WatchDirectoryEntries | Context: harmos.context.v1; client: harmos.v1 | Renamed; same directory test; preserve DirectoryEvent and rescan-required semantics. |

Samples.Append and Storage.AppendSamples have separate namespaces: the former
publishes a declared stream, the latter admits persisted sample batches. They are
not merged. Locked library namespaces include their service, for example
`harmos.context.v1.Storage/AppendRecords` or `harmos.v1.Storage/AppendRecords`; the exact namespace transition and fingerprints
migrate together. Public names in the table do not rename arbitrary
consumer-defined operation IDs behind their owners' backs.

Invocation carrier suboperations remain: opening request/terminal result, unary
child request/result, parent cancel/child cancel, and publication opening/ticks.
They are frames, not additional public RPCs. Keep catalog sections, requirements,
visibility, fingerprints, digest, descriptors and failure causes used in admission.

### Non-Storage Package Layout

The running-instance service is `harmos.plugin.v1.Instance` in
`proto/harmos/plugin/v1/instance.proto`, with Instance-qualified wrappers.
CatalogEntries, Invocations and Time have separate service files and independent
wrappers in `harmos.context.v1` and `harmos.v1`. Samples.Append has only a context
file. Reusable primitives, plugin catalog/control payloads and discovery values
live in service-free `harmos/v1/types.proto`; the discovery contract is
DiscoveredContract, distinct from the plugin catalog's OperationContract.
Rust exposes `plugin`, `context` and `v1` modules; the superseded `host` module is
removed. Python authoring lives in `authoring.py` and `invocation.py`; its generated
face namespace packages have inert initializers, preserving root SDK imports.
Storage face inventory and management services are integrated by their owners.
Runtime face admission remains independent of binding availability.

### Current Visibility Discrepancy

`crates/harmos-runtime/src/operations/manager/plugin_grpc.rs` applies the Private
peer check only when the selected provider has a plugin Binding. A host-provided
built-in still requires a matching declared requirement/fingerprint, but bypasses
that visibility gate. `entry.rs` rejects external Private invocation separately.
The existing private_refusal.rs story proves plugin-provider refusal, not
host-built-in refusal. This is not an intentional grant or a reason to weaken
Protected: task 0.9 must prove and enforce the locked host-built-in admission.

### Decided Built-In Visibility Mapping

**Owner Direction, 2026-11-12:** the following is the decided mapping, closing
`operation-visibility` task 1.7; its old two-level text describes the earlier
implementation, not the current direction. Today means the declarations under
`crates/harmos-operations/src/`; omitted VISIBILITY uses the Private default in
`execution/operations/registration/generated.rs`. The inventory includes all 41
built-ins plus reserved lifecycle/observer controls. Private means process-local
for the declaring host as well as for a declaring plugin; none of these existing
host-driven controls becomes invisible to its host by being relabelled Private.

| Current Built-In IDs | Today | Locked Level and Destination |
| --- | --- | --- |
| host.catalog | Public | Public: CatalogEntries.List on context/client faces. |
| host.operations.status, host.operations.cancel, host.operations.watch | Public | Public: Invocations.Get/Cancel/Watch; ownership checks remain. |
| harmos.clock.now, harmos.clock.sleep | Public | Public: Time.Get/Sleep on context/client faces. |
| host.publish | Public | Public: context Samples.Append; face restricts callers independently. |
| storage.capabilities, storage.open_snapshot, storage.release_snapshot, storage.scan_arrow, storage.scan_index | Public | Public: Storage.Capabilities/CreateSnapshot/DeleteSnapshot/ListColumnRows/ListIndexEntries. |
| storage.register_index, storage.register_projection, storage.advance_projection, storage.read_projection, storage.inspect_projection, storage.compare_exchange_object | Public | Public: Storage.SetIndexDefinition/SetProjectionDefinition/AdvanceProjection/ListProjectionRows/GetProjectionStatus/SetObject. |
| storage.query, storage.follow_records | Public | Public: Storage.ListRows/WatchRows. |
| storage.follow_stream, storage.follow_objects | Private | Public: selectors of the already locked shared context/client Storage.WatchRows; preserve storage grants. |
| storage.write_records, storage.write_streams, storage.write_objects | Public | Public: Storage.AppendRecords/AppendSamples/AppendObjectRevisions. |
| storage.write_file | Public | Public: shared Storage.CreateFileProduction/AppendFileBytes/PublishFileProduction workflow. |
| storage.create_file, storage.append_file, storage.resume_file, storage.finish_file, storage.abandon_file | Private | Public: shared Storage.CreateFileProduction/AppendFileBytes/CreateFileWriter/PublishFileProduction/DeleteFileProduction; preserve storage grants. |
| storage.read_file, storage.hash_file, storage.capture_file, storage.follow_file, storage.follow_production, storage.list_directory, storage.watch_directory | Public | Public: Storage.GetFileBytes/GetFileHash/CaptureFile/WatchFileBytes/WatchFileProduction/ListDirectoryEntries/WatchDirectoryEntries. |
| storage.status, storage.write_schema, storage.delete_file | Private | Protected: native-only Storage.Info/SetSchema/DeleteFile; host use only, no wire projection. |
| harmos_observe | Private | Protected: host-driven observer control on its owning plugin's carrier, no client binding. |
| harmos.initialize, harmos.shutdown | Private | Protected: host-driven plugin lifecycle controls, no client binding. |

New context/client built-ins in the generic machinery inventory are Public:
Plugins and its nested library services, access.Listener/access.Pairings/access.Certificates,
Status/status.Logs/status.TraceEvents/status.Diagnostics, Invocations.Invoke, Storage definition Get
mirrors (except native-only Protected GetSchema) and GetFileProduction. Context-only HostValues.Get is Public within its
face and resolver policy, like Samples.Append. Carrier methods are protocol
controls, not user operations with a visibility choice. A future host-only request
of a plugin must be declared Protected. Private remains available for local
implementation operations and is never exported to implement a host request.

Public permits routing; it does not grant storage or application authority.
The current Private file-session and follow helpers becoming Public replacements
is intentional in the previously locked shared transfer/read API; preserve their
request grants and admission checks. This mapping does not reinterpret today's
Private as the new local-only Private. No built-in mapping question remains open.

### Added Get Mirrors

| Added Library Operation (Owner: Library) | Exposure | Locked Value Contract |
| --- | --- | --- |
| Storage.GetIndexDefinition | Context harmos.context.v1; client harmos.v1 | Whole writable index definition and revision, mirrored by SetIndexDefinition. |
| Storage.GetProjectionDefinition | Context harmos.context.v1; client harmos.v1 | Whole writable projection definition and revision, mirrored by SetProjectionDefinition. |
| Storage.GetObject | Context harmos.context.v1; client harmos.v1 | Whole writable object and revision, mirrored by SetObject; explicit absence for creation. |
| Storage.GetSchema | Native only, no wire package, matching SetSchema | Whole writable schema and revision, mirrored by SetSchema. |
| Storage.GetFileProduction | Context harmos.context.v1; client harmos.v1 | Accepted/durable upload progress, writer generation and resume status. |

These small writable parts need no subdivision. Large application writable values
must instead map to separately settable named sub-resources,
each with its own Get/Set and revision, rather than a monolithic replacement.

### Added Generic Machinery Inventory

All destinations here are library-owned. Existing plugin-manager work supplies
Plugins; do not create a second implementation. Transfers share StorageManager,
not a second upload state or implementation. Each row specifies one face only.

| Package | Service | Methods | Face / Value Contract |
| --- | --- | --- | --- |
| harmos.v1 | Plugins | Create, Get, List, Watch, Set, Restart, Delete | Client; managed/remote entries, verified bundles, persistence, activation state and failures; revision-guarded whole-value Get/Set. |
| harmos.v1.access | Pairings | Create, List, Delete | Client; durable independent pairings under GrpcManager. |
| harmos.v1.access | Certificates | Create | Client; live trust regeneration under GrpcManager. |
| harmos.v1.access | Listener | Get, Set | Client; writable listener value/revision and separate runtime facts. |
| harmos.v1 | Status | Get, Watch | Client; identity, availability and contributed health. |
| harmos.v1.status | Diagnostics | Get | Client; bounded immutable archive ranges. |
| harmos.v1.status | Logs | List, Watch | Client; bounded log selector and positions. |
| harmos.v1.status | TraceEvents | List, Watch | Client; bounded trace selector and positions. |
| harmos.context.v1 | HostValues | Get | Context; declared-value resolution through the product-owned resolver/policy; no value store or client method. |
| harmos.v1 | Invocations | Invoke | Client; generic invocation of an admitted operation, including a Public plugin-served operation; operation ID, fingerprint and encoded payload, with caller scope inherited through the host. Adds one method beside Get/Cancel/Watch. |

### Complete Face Inventory and Shared Capabilities

This inventory is exhaustive for the library's wire declarations. Each listed
service has exactly one face. Equal short names in different packages are
separate services with separate wrappers, authorization and fingerprints.

| Face / Package | Service | Operations | Reason |
| --- | --- | --- | --- |
| Plugin protocol / harmos.plugin.v1 | Instance | Get, Invoke, Samples, Stop | One running instance, host callers only; Samples observes publication demand, not historical reads. |
| Context / harmos.context.v1 | CatalogEntries | List | Plugin requirement discovery through its context. |
| Context / harmos.context.v1 | Invocations | Get, Cancel, Watch | Plugin observes/controls its owned child invocations through context. |
| Context / harmos.context.v1 | Time | Get, Sleep | Authoritative host clock during plugin work. |
| Context / harmos.context.v1 | Samples | Append | Plugin publishes declared live samples; storage ingestion remains separate. |
| Context / harmos.context.v1 | Storage | The 32 operations enumerated below | Preserve authorized plugin storage access and transfers as child calls. |
| Context / harmos.context.v1 | HostValues | Get | Invocation-scoped declared values, unavailable to clients. |
| Client / harmos.v1 | CatalogEntries | List | Client discovery. |
| Client / harmos.v1 | Invocations | Invoke, Get, Cancel, Watch | Generic invocation and owned-call lifecycle. |
| Client / harmos.v1 | Time | Get, Sleep | Host-clock reads/waits for client callers. |
| Client / harmos.v1 | Storage | The 32 operations enumerated below | Authorized storage reads/mutations/transfers. |
| Client / harmos.v1 | Plugins | Create, Get, List, Watch, Set, Restart, Delete | Entry management, distinct from a running instance. |
| Client / harmos.v1.access | Pairings | Create, List, Delete | Durable independent client credentials. |
| Client / harmos.v1.access | Certificates | Create | Live trust rotation. |
| Client / harmos.v1.access | Listener | Get, Set | Nested access resource. |
| Client / harmos.v1 | Status | Get, Watch | Host status. |
| Client / harmos.v1.status | Diagnostics | Get | Bounded archive ranges. |
| Client / harmos.v1.status | Logs | List, Watch | Nested status resource. |
| Client / harmos.v1.status | TraceEvents | List, Watch | Nested status resource. |

Capabilities on both context and client faces are declared once per face, never
one service shared by both audiences. This preserves existing plugin-callable
capabilities rather than silently narrowing them. The complete duplicate list is:

| Services on Both Faces | Operations per Face | Why Both Need Them |
| --- | --- | --- |
| CatalogEntries | List (1) | Both discover contracts; plugin requirement admission still applies. |
| Invocations | Get, Cancel, Watch (3) | Both observe/control their owned calls, independently authorized. |
| Time | Get, Sleep (2) | Both read/wait on the host clock, never an SDK wall clock. |
| Storage | Capabilities, CreateSnapshot, DeleteSnapshot, ListColumnRows, ListIndexEntries, SetIndexDefinition, SetProjectionDefinition, AdvanceProjection, ListProjectionRows, GetProjectionStatus, SetObject, ListRows, WatchRows, AppendRecords, AppendSamples, AppendObjectRevisions, CreateFileProduction, AppendFileBytes, CreateFileWriter, PublishFileProduction, DeleteFileProduction, GetFileBytes, GetFileHash, CaptureFile, WatchFileBytes, WatchFileProduction, ListDirectoryEntries, WatchDirectoryEntries, GetIndexDefinition, GetProjectionDefinition, GetObject, GetFileProduction (32) | Plugins append/read the data they produce and use bounded queries, native-backed selections and resumable transfers; clients consume/manage the same authorized data. Preserve the existing capability set, grants, receipts, retention and shared handlers, with separate face contracts. |

There are 38 shared capabilities, hence 76 face-local declarations. Context
HostValues.Get and Samples.Append have no client counterpart. Client entry,
access/status management and Invocations.Invoke have no context counterpart.
The context's generic child-call frame routes a declared requirement through the
one host registry; it adds no context Invoke RPC or per-provider client service.
Product plugin operations remain catalog declarations carried by Instance.Invoke,
not extra library methods or client service declarations.

### Library-Only Counts

Counts are templates, not instances. Context service descriptors declare method
signatures and face-local wrappers, carried by child frames; they project zero
standalone gRPC methods and are never registered on the client endpoint. The
gRPC column counts declared descriptor methods and states standalone exposure
where different; only client methods and instance carriers open independent calls.
Shared primitives, domain payloads, frame alternatives and imported protobuf
messages are excluded from wrapper totals and require a separate descriptor census.

| Face / Package | Services | Operations / Method Signatures | gRPC Methods | Requests | Responses |
| --- | ---: | ---: | ---: | ---: | ---: |
| Plugin protocol / harmos.plugin.v1 | 1 | 4 | 4 | 4 | 4 |
| Context / harmos.context.v1 | 6 | 40 | 40 (0 standalone) | 40 | 40 |
| Client / harmos.v1 and nested packages | 12 (6 nested) | 59 | 59 | 59 | 59 |
| All wire faces | 19 (6 nested) | 103 | 103 (63 standalone) | 103 | 103 |
| Native storage hooks, no wire package or service | 0 | 4 (not method signatures) | 0 | 4 | 4 |
| Whole library inventory | 19 (6 nested) | 107 | 103 (63 standalone) | 107 | 107 |

Native Storage.Info, SetSchema, GetSchema and DeleteFile remain Protected native-only
operations/hooks, not a fourth wire face or additional service. Their existing
native request/result shapes follow the wrapper vocabulary but are not emitted
as face methods. Storage has 32 declarations on each wire face plus four native
hooks: 68 inventory entries, preserving 36 underlying capabilities. StorageBackend
remains 58 to 52 hooks and StateBackend 8 to 8, or 66 present / 60 target hooks.

Arithmetic: context 40 = 1 catalog + 3 controls + 2 clock + 32 storage
+ 1 publication + 1 values. Client 59 = 1 catalog + 4 invocations + 2 clock
+ 32 storage + 7 entries + 6 access + 7 status. The owner's nested access/status
instruction adds one net method to the earlier inventory. Protocol 4 + context 40
+ client 59 = 103 face operations and 206 face wrappers; four native hooks add
8 wrappers, for 214 overall. Context signatures remain child-carried; 63 methods
open standalone calls. Namespace parents Access and status add no empty services.

The merged descriptor census matches the table: 23 library proto files and three
imported protobuf files; 206 distinct method request/response messages, eight
native hook wrappers, 157 other library payload messages and 37 imported messages
(408 messages including nested messages/map entries), plus 19 library and 21
imported enums. The 38 shared capabilities are CatalogEntries.List, three
Invocations controls, two Time methods and 32 Storage methods, each independently
declared on the context/client faces. No automatic twins survive. ReadFile and
WriteFile declarations, handlers and requests are deleted following the owner's
2026-11-13 throughput review. A promised cooperative final-result replacement
changes no method count. Instance.Samples is a method, not a nested service;
namespace-only parents never create empty services.

### Protocol and Context Proto Examples

These are separate illustrative files with their own face-local wrappers. The
existing carrier frame contents remain protocol payloads, including child
requests/results and cancellation. A wrapper is counted once per method even
when that method streams many frames.

```proto
// proto/harmos/plugin/v1/instance.proto
package harmos.plugin.v1;
message InstanceGetRequest { uint32 revision = 1; }
message InstanceGetResponse {
  string id = 1;
  Catalog catalog = 2;
  // Version, start time and negotiated protocol/SDK identity remain included.
}
message InstanceInvokeRequest { HostFrame frame = 1; }
message InstanceInvokeResponse { PluginFrame frame = 1; }
message InstanceSamplesRequest { PublicationFrame frame = 1; }
message InstanceSamplesResponse { SampleFrame frame = 1; }
message InstanceStopRequest { string reason = 1; }
message InstanceStopResponse {}
service Instance {
  rpc Get(InstanceGetRequest) returns (InstanceGetResponse);
  rpc Invoke(stream InstanceInvokeRequest) returns (stream InstanceInvokeResponse);
  rpc Samples(stream InstanceSamplesRequest) returns (stream InstanceSamplesResponse);
  rpc Stop(InstanceStopRequest) returns (InstanceStopResponse);
}
enum Visibility {
  VISIBILITY_UNSPECIFIED = 0; // Invalid: rejected, never a default policy.
  VISIBILITY_PRIVATE = 1; // Local declaration only; forbidden in a wire catalog.
  VISIBILITY_PUBLIC = 2; // Other plugins, host and host clients too.
  VISIBILITY_PROTECTED = 3; // Declaring plugin and host; carrier only.
}
message OperationContract {
  string id = 1;
  string request = 2;
  string response = 3;
  Visibility visibility = 4; // Explicit; only Protected/Public may be catalogued.
  OperationPolicy policy = 5;
  bytes fingerprint = 6;
}
message Requirement { string id = 1; bytes fingerprint = 2; }
message Catalog {
  repeated OperationContract operations = 1;
  repeated Requirement requires = 2;
  // Streams, sections, digest and descriptors remain required as today.
}
```

```proto
// proto/harmos/context/v1/time.proto
package harmos.context.v1;
import "google/protobuf/timestamp.proto";
import "google/protobuf/duration.proto";
message TimeGetRequest {}
message TimeGetResponse { google.protobuf.Timestamp now = 1; }
message TimeSleepRequest { google.protobuf.Duration duration = 1; }
message TimeSleepResponse {}
// Descriptor signatures for context child calls; not a client endpoint service.
service Time {
  rpc Get(TimeGetRequest) returns (TimeGetResponse);
  rpc Sleep(TimeSleepRequest) returns (TimeSleepResponse);
}
```

```proto
// proto/harmos/context/v1/storage.proto (excerpt)
package harmos.context.v1;
message StorageAppendRecordsRequest {
  string parent = 1;
  string request_id = 2;
  repeated harmos.v1.Record records = 3;
}
message StorageAppendRecordsResponse { harmos.v1.Receipt receipt = 1; }
service Storage {
  rpc AppendRecords(StorageAppendRecordsRequest) returns (StorageAppendRecordsResponse);
  // Other mapped signatures have their own context wrappers as well.
}
```

The client time/storage excerpts above use harmos.v1 wrappers, never these context
wrappers. Context CatalogEntries, Invocations, Samples and HostValues follow the
same per-service file and wrapper rule. Their methods route as child calls during
Instance.Invoke, preserving inherited scope and cancellation; the host's context
dispatch does not open an independent context gRPC channel. Requirement IDs may
also resolve a Public plugin-provided operation in the same registry without
introducing another service. Only the host opens the provider's Instance.Invoke.

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
// proto/application/v1/plugins/settings.proto (product-owned)
package application.v1.plugins;
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

Status.TraceEvents follows the same per-service file,
subpackage and wrapper rule. Context-only wrappers belong to harmos.context.v1 and are carried by child calls;
no client service or client root is generated for them.

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
library protocol, SDKs and conformance checks remain self-sufficient, with no
product vocabulary. Moving SDKs into a product was considered and not chosen
because a second plugin-hosting product would have to reimplement the plugin
protocol in every language and the library could not be proven end to end by itself.
Current SDKs contain substantive protocol behavior, not merely protobuf bindings.

| Language | Library SDK Contains and Consumers |
| --- | --- |
| Rust | Descriptor/fingerprint authoring, builders, invocation resources, server, clock/publication, guest/native support, generic managed application client and 11 storage request bindings. Library examples, fixtures and runtime tests. |
| Python | Plugin decorators/descriptors, fingerprint validation, server/context, resources, clock/publication and generated protobuf/Storage stubs. Python fixtures and SDK tests. |
| C# | Plugin declarations, fingerprinting, service/context, child calls, clock/publication and generated build-time protocol clients. C# fixture and SDK checks. |
| C++ | Plugin builders, schema/fingerprints, gRPC server/context, resources, clock/publication and generated build-time protocol stubs. C++ fixture/tests. |

Measured on 2026-11-12 from tracked source files only (`.rs`, `.py`, `.pyi`, `.cs`,
`.cpp`, `.hpp`, `.h`); counts include tests, examples and source tooling, exclude
locks/configuration and untracked build output. These are physical lines and file
bytes, not package download size.

| Language | Library Files / Lines / Bytes |
| --- | --- |
| Rust | 72 / 10,643 / 349,939 |
| Python | 25 / 4,590 / 216,097 |
| C# | 10 / 1,113 / 44,042 |
| C++ | 12 / 1,535 / 54,670 |

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
open under SDKs; four-language managed-invocation parity is not a landed fact or an approved
new requirement. Descriptor-derived client-root naming in all four languages is
separately locked.

### Defects to Fix Regardless

| Defect | Verification | Locked Independent Correction |
| --- | --- | --- |
| Python defaults to Public; other SDKs default to Private | `sdk/python/src/harmos/contracts.py`, Rust operation declarations, C# Operation and C++ header. | Require explicit Private, Protected or Public in Rust, Python, C# and C++; no SDK default. Keep Private local and out of catalogs. Reject omitted/unspecified visibility at admission. |
| C# lacks invocation-local typed resources and returned cancellation reason | `sdk/csharp/Harmos/Context.cs`: internal cancelReason exists, CancelledAsync returns Task only; no public resource store. | Add per-invocation resource ownership and reason-bearing cancellation, with child settlement parity. These are not persisted settings. |
| Static storage service exposes a subset | 11 proto RPCs versus 34 authoritative storage declarations and 11 Rust request bindings. | Generate the exposed service/client contracts from one declaration owner; preserve grants and native-only boundaries. |
| Retry policy has two shapes | Plugin attempts/fixed backoff versus host max_retries/fixed-or-exponential/jitter; plugin declaration lacks replay safety. | One policy and replay classification, with call-local overrides. |
| Plugin proto says draft/unimplemented | First line of `proto/harmos/v1/plugin.proto` conflicts with four implemented SDKs and host runtime. | Replace the stale header with the current contract status during implementation. |

Scope accessors, inherited scope, client retained-byte admission and descriptor-only
message registration already exist here. Do not reimplement them based on stale
sibling pins. Typed operation discovery already fills CatalogPage.operations;
stored schema/identity catalogs remain JSON bytes and need a typed replacement.

Generic plugin, access, transfer and status gaps are library work under the
manager inventory above; application operations remain outside this repository.

## Plugin Manager

Owner decision, 2026-11-12: move generic plugin management into the library.
`harmos.v1.Plugins` is the client-facing resource service;
`harmos.plugin.v1.Instance` is the distinct service implemented by a plugin
executable. This section specifies the seven client operations and fourteen
method wrappers counted in the locked inventory above.

```proto
service Plugins {
  rpc Get(PluginsGetRequest) returns (PluginsGetResponse);
  rpc List(PluginsListRequest) returns (PluginsListResponse);
  rpc Watch(PluginsWatchRequest) returns (stream PluginsWatchResponse);
  rpc Create(PluginsCreateRequest) returns (PluginsCreateResponse);
  rpc Set(PluginsSetRequest) returns (PluginsSetResponse);
  rpc Delete(PluginsDeleteRequest) returns (PluginsDeleteResponse);
  rpc Restart(PluginsRestartRequest) returns (PluginsRestartResponse);
}
message PluginValue { bool enabled = 1; }
message PluginDescription {
  string id = 1;
  string version = 2;
  google.protobuf.Timestamp started_at = 3;
  harmos.v1.Catalog catalog = 4;
  uint32 revision = 5;
  string sdk = 6;
  string sdk_version = 7;
}
message PluginRemote { string url = 1; }
message PluginManaged {
  harmos.storage.v1.FileReference content = 1;
  bytes sha256 = 2;
}
message PluginPlacement {
  oneof kind { PluginManaged managed = 1; PluginRemote remote = 2; }
}
enum PluginState {
  PLUGIN_STATE_UNSPECIFIED = 0;
  PLUGIN_STATE_CONNECTING = 1;
  PLUGIN_STATE_READY = 2;
  PLUGIN_STATE_UNAVAILABLE = 3;
  PLUGIN_STATE_STOPPED = 4;
  PLUGIN_STATE_STOPPING = 5;
}
message PluginProcess {
  uint32 id = 1;
  bytes executable_digest = 2;
  google.protobuf.Timestamp started_at = 3;
}
message PluginEntry {
  string name = 1;
  PluginPlacement placement = 2;
  PluginValue value = 3;
  bytes revision = 4;
  PluginState state = 5;
  harmos.v1.Failure failure = 6;
  PluginDescription description = 7;
  bytes catalog_digest = 8;
  PluginProcess process = 9;
  uint64 activation = 10;
}
message PluginsGetRequest { string name = 1; harmos.v1.ReadCut at = 2; }
message PluginsGetResponse { PluginEntry entry = 1; harmos.v1.ReadCut resolved_at = 2; }
message PluginsListRequest { harmos.v1.ReadCut at = 1; }
message PluginsListResponse {
  repeated PluginEntry entries = 1;
  harmos.v1.ReadCut resolved_at = 2;
}
message PluginsWatchRequest { string name = 1; harmos.v1.ReadCut at = 2; }
message PluginSnapshot { repeated PluginEntry entries = 1; }
message PluginChange { oneof kind { PluginEntry entry = 1; string deleted = 2; } }
message PluginsWatchResponse {
  harmos.v1.ReadCut resolved_at = 1;
  oneof event {
    PluginSnapshot snapshot = 2;
    PluginChange change = 3;
    string resync_required = 4;
  }
}
message PluginsCreateRequest {
  string request_id = 1;
  string name = 2;
  PluginPlacement placement = 3;
  PluginValue value = 4;
}
message PluginsCreateResponse { PluginEntry entry = 1; harmos.v1.Receipt receipt = 2; }
message PluginsSetRequest { string name = 1; PluginValue value = 2; bytes expected_revision = 3; }
message PluginsSetResponse { PluginEntry entry = 1; harmos.v1.Receipt receipt = 2; }
message PluginsDeleteRequest { string name = 1; bytes expected_revision = 2; }
message PluginsDeleteResponse { harmos.v1.Receipt receipt = 1; }
message PluginsRestartRequest { string name = 1; bytes expected_revision = 2; }
message PluginsRestartResponse { PluginEntry entry = 1; }
```

```rust
use harmos_runtime::plugin::manager::{Manager, PluginManagerSettings};
use std::sync::Arc;
// The host supplies storage, its installation root, all capacities and policy.
let manager = Manager::new(storage, content, settings)?;
setup.settings().plugins(manager.clone());
let runtime = Arc::new(/* launch the configured host */);
manager.start(&runtime).await?;
// Management calls use registered operations and normal authorization.
// Plugin invocation uses the existing scope-preserving operation handle.
let operations = runtime.operations.with_scope("opaque-scope")?;
let reply = operations.call_encoded("example.echo", payload).await?;
```

`Manager` implements `Management`, whose typed `get`, `list`, `watch`, `create`,
`set`, `delete` and `restart` methods take the generated requests above and return
`Pending<'_, Response>`; `watch` returns a bounded receiver of generated responses.
`Manager::new` takes the runtime `StorageManager` and `PluginManagerSettings`.
The latter owns the namespace, directory, target,
existing placement and bundle policies, and all registry/read/queue capacities.
Bundle installation opens `StorageManager::open_content`, retaining one bounded
`ContentRead` while assembling and verifying the configured archive. Library-owned
files and registered application sources share the storage-owned contract below.

The runtime builder's `plugins(Manager)` registers the resource and catalog;
`start(&Arc<Runtime<A>>)` starts restoration and host-owned supervision once.

| Decision | Reason |
| --- | --- |
| Client-only `harmos.v1.Plugins` and its own description payload | Instance method wrappers remain on the plugin face; the client receives observed identity/catalog data without importing a carrier service. |
| Immutable placement and one writable `enabled` value | Set cannot turn an installed executable into an unrelated remote endpoint; replacement uses Delete/Create. |
| Unchanged Set is a no-op | Re-establishing the same writable value preserves its revision and activation rather than restarting a healthy plugin. |
| Stopping precedes Stopped when disabling | Watch reports durable disabled intent while cleanup runs; Set completes after process settlement and the final stopped state is saved. |
| Caller-chosen opaque entry name and replay-safe Create request identifier | Identity survives reconnect and host restart independently of catalog identity; normalized replay returns the original response, and changed input conflicts. |
| Durable registry object using conditional storage replacement | One bounded registry atomically owns entries and Create receipts without a private database or cross-object transaction. Consumer capacities bound entries, receipts and encoded registry bytes; exhausted receipt retention refuses new creates instead of silently expiring identities. |
| Verify uploaded content and existing bundle format before admitting an entry | Digest, manifest, native target and executable checks precede extraction and process startup. The manager owns the installed directory; clients never supply executable paths. |
| One storage-owned content boundary | Bundle installation, diagnostics and ranged reads share ContentSource/ContentRead; the manager never creates a second upload protocol or content-source trait. |
| Reuse attachment connection, admission, invocation, streams and supervision | The manager owns the set and lifecycle, while `attach_grpc` remains the single hosting implementation. |
| Retain unavailable entries and the last connected description | A failed process or endpoint must remain manageable and diagnostically visible. Restart replaces the activation; disabled entries remain stopped. |
| Reconnect remote entries and start enabled managed entries on startup | Persisted intent survives a host restart; observed process state is reconstructed, never trusted from disk. |
| Issued cuts and bounded Watch events | List returns the complete configured entry set in identifier order from one durable manager snapshot. Watch opens with a snapshot, then ordered changes; overflow reports explicit resynchronization instead of skipping evidence. |
| Manager-instance cut epoch | Issued cuts expire across host restart; stale or foreign positions fail explicitly. Durable entry identities and revisions survive. |
| Product policy through existing visibility, grants and external authorizer | Opaque invocation scope is inherited unchanged and never parsed as authority; no product role model enters the manager. |
| No automatic process restart loop | Remote reconnect remains attachment policy; failed managed processes stay unavailable until Restart or host restart, avoiding a second restart scheduler. |
| Hidden Windows process startup and explicit loopback endpoint environment | Managed executables have no console window and receive `HARMOS_ADDRESS=127.0.0.1:0`; existing readiness announcement supplies the bound endpoint without reserving a racy port. |
| Explicit startup against `Arc<Runtime<A>>` | The manager retains a weak host reference, avoiding a runtime/resource cycle; normal runtime shutdown owns and awaits its participant. |

Owned source inventory: `proto/harmos/v1/plugins.proto`,
`crates/harmos-runtime/src/plugin/manager/**`,
`crates/harmos-operations/src/plugins/**`, manager stories under
`crates/harmos-runtime/tests/plugin_grpc/manager/**`,
descriptor-generated SDK client roots, and `docs/plugin-manager.md`.
Existing hosting extensions are confined to `plugin/grpc/mod.rs` (shared
attachment supervision, process identity and awaited reaping after disconnect), `plugin/grpc/managed.rs` (explicit
endpoint and platform launch), and `plugin/grpc/managed/{windows,unix}.rs`
(platform command construction). Other source edits are registrations and
generated protocol output, plus seven built-in contract registrations in
`crates/harmos-operations/src/operations/contract.rs`. The shared registry validates
the exact protocol, binding and public visibility without a manager-specific
reserved-namespace exception. Shared primitives and carrier renames join from
`feat/gp-grammar-core`; unary uploads and ranged content join from the storage
worker. Neither parallel owner's semantics are reimplemented here.

The settings/secrets ownership question was initially open. The owner's locked
boundary decision resolves it: product-owned stores, schemas, UI and credentials;
library-owned settings declarations and context host-value delivery under their
existing owners. The manager adds neither a second resolver nor a settings store.
The catalog's meaning and which plugins a unit of work uses remain with the product.

## Risks and Trade-offs

### Access Client Services

The owner's access/status implementation instruction supersedes the earlier inventory's
compound Access actions and split Status reads. Namespace parents have no empty service.

```proto
package harmos.v1.access;
service Listener {
  rpc Get(ListenerGetRequest) returns (ListenerGetResponse);
  rpc Set(ListenerSetRequest) returns (ListenerSetResponse);
}
service Pairings {
  rpc Create(PairingsCreateRequest) returns (PairingsCreateResponse);
  rpc List(PairingsListRequest) returns (PairingsListResponse);
  rpc Delete(PairingsDeleteRequest) returns (PairingsDeleteResponse);
}
service Certificates {
  rpc Create(CertificatesCreateRequest) returns (CertificatesCreateResponse);
}
// ListenerValue: enabled, endpoint, security (TLS names).
// Get/Set: complete value, opaque revision; Set requires expected_revision.
// Pairing: id, label, endpoint, pinned_certificate_pem, credential (Create only).
```

```rust
grpc.configure_access(settings, credential_store).await?;
grpc.access()?; // The gRPC manager retains the access control and listener lease.
trait CredentialStore: Send + Sync {
    fn load(&self) -> Pending<'_, Option<Credentials>>;
    fn commit(&self, expected_revision: u64, value: Credentials) -> Pending<'_, ()>;
}
```

Decisions: listener revisions change only when replacing its writable value; a separate
credential-store revision serializes all atomic transactions. Pairings are durable independent credentials, so List and Delete are necessary
for discovery and revocation; List never returns a secret. One atomic store transaction
replaces listener settings, certificate/key and pairing hashes together, so an unavailable
store cannot partly rotate or configure access. The host supplies this store, including
its settings layout and compare/exchange semantics. Certificate replacement validates
before persistence and reuses ServerCertificate's live resolver; rotation revokes all
pairings while existing TLS connections remain alive and must use a new credential.
Authentication inserts `access::Principal { pairing_id }` before application middleware
and operation authorization, so policy can use an authenticated identity without
duplicating credential checks. Only the dedicated access listener requires bearer credentials; an application may retain
a separately authorized management listener to bootstrap and recover disabled access.
Listener changes stage a replacement before commit, then retire the old attachment;
same-endpoint security-name changes require Certificates.Create rather than a restart.
Errors name Listener.Get/Set, Pairings.Create/List/Delete or Certificates.Create.

### Status Client Services

```proto
package harmos.v1;
service Status {
  rpc Get(StatusGetRequest) returns (StatusGetResponse);
  rpc Watch(StatusWatchRequest) returns (stream StatusWatchResponse);
}
// Response: host/application/library identity, process start, availability,
// named health components (state/reason), resolved_at.
// package harmos.v1.status; one file per service:
// Logs.List/Watch, TraceEvents.List/Watch, Diagnostics.Get.
// List: optional at, PageRequest; reply: events, resolved_at, next_page_token.
// Watch: optional at; reply: opening boundary, changes or explicit resync failure.
// Diagnostics.Get: archive id, offset, max_bytes; data, total_length, sha256.
```

```rust
services.configure_status(settings)?;
let registration = services.status()?.contribute(provider)?;
trait Contribution: Send + Sync {
    fn health(&self) -> Result<Vec<harmos_protocol::v1::HealthComponent>,
        Box<dyn std::error::Error + Send + Sync>>;
    fn diagnostics(&self) -> Result<Vec<services::status::Entry>,
        Box<dyn std::error::Error + Send + Sync>>;
}
// A retained registration owns contribution lifetime; update notifies watchers.
// Status's log/trace recording interface and tracing Layer redact before retention.
```

Decisions: Status.Watch is required to observe contributed health changes without polling;
it delivers the latest coalesced status, whereas event watches retain ordered batches.
One frozen diagnostics archive is retained; collecting another explicitly expires the old
content name rather than adding an unbounded content cache.
Host identity includes the host-supplied stable identifier; runtime captures its own clock
start and library version. Contributions have host-owned registration lifetime and explicit
change notification; provider failure appears as unavailable health and fails diagnostics
with the action named. Histories are bounded by consumer item/byte budgets. List pins
an issued process-history position; tokens bind kind, boundary and exclusive offset.
Watch opens atomically at a boundary and follows later events, with explicit expiry/gap
failure instead of silently skipping. Process histories expire at restart and retention
eviction; zero is an issued empty boundary, never an arbitrary historical selection.
Logs/traces are volatile diagnostics, not claims of storage durability.
Diagnostics freezes a bounded tar archive under an opaque content id; subsequent ranges
read the same bytes and digest. The shared storage `ContentRead` mechanism serves archive and stored-file
ranges, including short reads and digest verification; no streaming twin or upload engine is added.
Text-only contributed entries permit redaction before archiving; reject unsafe/duplicate
archive paths and refuse over-budget archives instead of truncating valid content.
Registered secret values and sensitive field names redact before log/trace retention,
status reasons and diagnostics serialization; credentials and keys never enter archives.

Ownership: grpc/access/**; services/status/**; operations/access/** and status/**;
proto/harmos/v1/access/**, status.proto and status/**; five access/status stories in tests/grpc/access and tests/services/status,
with tests/support/access_status.rs;
docs/access-status.md. Coupling extensions: GrpcManager retains access and closes its
attachment; ServiceManager retains status in its shared host resources; operation-manager
resource provision supplies narrow access/status ports; protocol generation includes new
client files/reflection; Rust generated roots consume their descriptors. Existing listener,
resolver, authorization, operation policy and bounded output mechanisms remain authoritative.

GrpcManager extensions are `configure_access` (one serialized setup), `access`
(the retained control), internal `attach_access` (per-listener authentication before
application middleware), and access settlement during shutdown. ServiceManager adds
`configure_status` and `status`; its shared operation registry retains one status owner,
its start time and narrow ports across manager handles. Operation-manager coupling is
limited to those provided resources, the status shutdown notification and exact
reserved-contract admission for the new protocol-owned operations. The listener and
ServerCertificate resolver themselves are reused unchanged.

- Receipt/position renames change fingerprints and admission: migrate descriptors,
  requirements and callers as one revision; never add silent compatibility aliases.
- Latest durable deliberately omits newer accepted data. Synchronous flush duration
  grows with payload size on top of cadence. The owner accepted the measured lag on
  2026-11-13; no accepted-data snapshot is built. Continue per-load measurements
  without a failing threshold, expose durability explicitly and retain writer barriers.
- Current follows can emit accepted data: changing them requires explicit durable
  boundaries and deterministic gap/reconnect proof, not a name-only edit.
- Unary sessions add round trips and remove the one-shot path on providers lacking
  sessions: document capability refusal and measure chunk overhead without changing
  consumer-owned concurrency/size budgets.
- Retained cuts and deletion can conflict: owner review must settle reader lifetime
  before removing or reclaiming content. No stored-data deletion is authorized here.

## Open Questions

Catalog discovery now returns the complete authorized catalog in one response, including
complete descriptors. `external_authority_parity` lists once; the carrier ceiling alone
guards the response and an oversize refusal names CatalogEntries.List.

Integration repair: automatic Time/HostValues contract admission propagates
schema-budget failures through runtime startup instead of panicking. The schema
snapshot story retains its one-byte refusal and verifies failure at startup when
even the automatically registered contracts cannot fit. Native request selection
resolves request identity before validating supplied endpoints, preserving typed
endpoint refusal and charged-request release. Retry fixture declarations explicitly
state their idempotence; stream-transfer guards still prevent replay after progress.

Integration decision: core service/method operation identities, complete SDK generation,
visibility and descriptor-derived roots are authoritative. Plugins, access/status
and committed storage declarations use those identities without aliases. The generator
isolates each package output and maps ancestor payloads individually, so imported
services cannot overwrite messages or suppress nested-package definitions.
Nested status messages select the C# namespace `Harmos.V1.StatusResources` to avoid
the generated Status service class collision. The common table reads protobuf C#
namespace options; resource paths still derive only from wire packages and names.
C++ resource identifiers escape language keywords with a trailing underscore. The
generator temporarily saves and undefines method-name platform macros while emitting
its calls, then restores them, retaining the original wire names.
Explicit face-local declarations take precedence over inferred context projections
within a registration batch; storage retains its own context handlers and grants.
Manager uploads use client CreateFileProduction/AppendFileBytes/GetFileProduction/
PublishFileProduction; abandonment remains DeleteFileProduction in the shared service.
Published bundles retain one storage snapshot and use its get_file_bytes ranged reader,
verifying the assembled reference digest before bundle admission. Diagnostics keeps service-owned
archive retention and uses the same storage ranged-content implementation as snapshots.
The second storage convergence merges through `8babe0a5`, including durable
WatchRows, file/directory watches, metadata continuations, native schema exchange,
consumer policy and bounded physical chunks. Runtime registration uses one typed
set of explicitly declared identities before inferring context projections. Wire
identity admission remains the core validator; the storage-only admission shortcut
stays deleted. Remaining joins are receipt-bearing log Append, retained-reader
DeleteFile reclamation and the cooperative final-result inventory; see tasks
I.8, 3.10 and 3.11. No whole-change source closure or cross-platform validation is
claimed by this merge.


Implementation choice (4.1): one descriptor table drives generated Rust/C#/C++ resource classes and Python runtime binding; parents and service names determine every client path.

Implementation (0.3–0.7): each SDK runs Private handlers from its local registry,
with fresh attempt resources, host-clock policy guards, replay-safe retries and
reason-bearing cancellation. Conformance cases 24 and 26–29 guard local isolation,
retry, deadline, cooperative cancellation and host-mediated peer visibility.
Child demand frames bound context Watch to one requested item; background policy
guards do not make active handler work eligible for virtual-time advancement.
The runtime peer story independently guards inherited scope, failure cause and
caller-to-child cancellation. HostValues.Get uses an OperationManager resolver
registration with declared value types and invocation scope; dropping registration
ends new resolution. No library settings or secrets store is introduced.

Implementation (amended by the owner on 2026-11-13): the Python audience build
hook/environment selector, Rust audience features, MSBuild/CMake switches and
per-audience generated-stub guards/tests are deleted. Each language's flat
`install_itself` entry program registers its own running remote instance with
Plugins.Create(enabled=true), awaits Ready through Watch, invokes its Public
example.echo through the same package's client root, then removes the entry.
The codebase serves the instance in a separate process through the ordinary plugin
API; installing it starts the host activation, not an unauthorized process launcher.
The examples live beside the four conformance programs and their runtime journeys
use the controlled host clock and awaited readiness/events, without polling or sleeps.

Implementation (4.6): context message descriptor graphs are projected during
generation and embedded per SDK message. Guest registration never initializes the
full host reflection pool; the existing fuel budgets remain unchanged. Guest
assignments retain their provider alongside the canonical service/method identity,
and runtime dispatch checks that provider, generation, fingerprint and audience.


Implementation decision (3.11): no library built-in promises a cooperative final result after cancellation: actions use unary receipts/status and watches end observation; explicit application `Grpc::managed()` declarations replace the canonical method. The Collect and Exchange fixtures promise the accepted-input aggregate, and Wait promises its typed cancellation summary; those three fixture declarations exercise the opt-in contract.

Implementation decision (4.1): Rust, Python, C# and C++ promise the same native plugin semantics, proven by the common conformance suite; Wasm remains Rust-only.

Implementation decision (4.2): the generic managed application client remains Rust-only; other SDKs use generated application gRPC clients.

Only these questions remain open. Settle each when implementation reaches its
owner; they do not block the other locked work. Each has a corresponding task.

1. **Issued Positions and Restart:** settle returned-position/page-token lifetime,
   expiry, provenance and retained composite selections across restart. Arbitrary
   earlier positions remain refused; product presentation retention stays there.
2. **External Watch Semantics:** the Durable Row Watch Contract below settles
   WatchRows. Migrate external file/directory watches while preserving their native
   replacement, truncation, production fencing, scan-boundary and gap contracts;
   runtime sources belong to their owning core task.
3. **Deletion With Retained Readers:** settle reader lifetime and reclamation on
   supported backends before implementing deletion changes.

## Migration Plan

The tasks inventory fixes the dependency order. First face package layout,
per-service file moves and import ownership; then shared primitives and final
descriptor/service names; then mechanically generated wrappers and hierarchical roots, migrating each affected producer and consumer in one compiling slice.
Integrate existing plugin-manager work rather than duplicate it. Add Access under
GrpcManager, Status and product-fact registration under ServiceManager, and the
generic host-value mechanism under OperationManager. Reuse Storage sessions and
the existing StreamManager. Preserve stored encodings; update conformance,
examples and affected guidance with the corresponding code.

Under later authorization publish one exact library revision for consumers to pin.
Host applications compose library services directly and retain their definitions,
objects and policy. Remove remaining superseded library code only after consumers
migrate and Nightly large-file throughput is measured. No compatibility aliases,
second long-lived authorities, implicit data migration or stored-data reset.
Rollback uses a matched prior host/SDK revision. Final validation remains separate
from source closure.

## Storage Implementation Ownership

`feat/gp-grammar-storage` owns `proto/harmos/v1/storage.proto`, `crates/harmos-storage/**`,
`crates/harmos-operations/src/storage/**`, storage handlers/tests/benches in runtime,
storage benchmark tooling and Nightly wiring, `docs/storage.md`, and the storage tasks/design
in this change. Shared protocol primitives and SDK bindings belong to the parallel core worker.
Streaming transfers were deleted by the owner's 2026-11-13 amendment; no
accepted-data snapshot is implemented.

### Windows Durable-Lag Measurements

Measured on 2026-11-13 in the bench profile with SQLite WAL/FULL, five independent
fresh-store runs per load before and after the persistence changes. The unchanged
loads are 1,000 single 32-byte samples separated by 1 ms; 128 logical 4 MiB batches
of 64 samples; and 1,000 mixed batches with every sixteenth batch large. Existing
admission bounds, the 100 ms cadence, 1 MiB early trigger, 16 MiB atomic ceiling,
response/physical-chunk settings and deadlines are unchanged. An independent
observer times admission-return to passive saved-frontier notification; admission
backpressure is reported separately. No read requests a flush.

Data used the existing system temporary volume, C: (about 55 GB free when checked);
build output used D:. No temporary-volume relocation or workload tuning was used.
This session started no other compilation during the final timed loads. Activity
samples record generic compiler names/PIDs/CPU at each load's endpoints, not a
claim that every intervening process or I/O source was observed. Final repetitions
1–2 for small/large and 1 for mixed showed only Python task drivers; later endpoints
also showed cargo/rustc/link and cl/cmake work from other sessions. Baseline later
runs likewise overlapped compilation. Even a compiler-free endpoint pair did not
guarantee a low-latency FULL commit.

| Load | Before p95 Median (Minimum–Maximum), ms | Final p95 Values, ms | Final p95 Median (Minimum–Maximum), ms | Owner Disposition on 2026-11-13 |
| --- | ---: | --- | ---: | --- |
| `small_high_rate` | 100.49 (98.36–142.69) | 98.17, 98.77, 105.23, 103.49, 99.07 | 99.07 (98.17–105.23) | Accepted |
| `multi_megabyte` | 789.01 (207.37–2358.97) | 177.80, 288.81, 570.00, 391.28, 457.40 | 391.28 (177.80–570.00) | Accepted |
| `mixed` | 744.14 (225.92–1702.05) | 172.86, 321.45, 492.84, 346.45, 396.80 | 346.45 (172.86–492.84) | Accepted |

Stage totals below are medians across the five final runs. Cadence and ordered
waits are means per admission; all other times are totals per fixed workload.
Stage counts differ, so these columns are not additive percentile decompositions.
Commit-byte counters are checked encoded upper bounds, not device traffic.

| Load | Cadence Mean, ms | Ordered Wait Mean, ms | Batch Build Total, ms | Provider Writing Total, ms | FULL Commit Total, ms | Commit Count | Publication Total, ms |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| `small_high_rate` | 51.545 | 0.006 | 0.706 | 29.685 | 28.923 | 23 | 0.048 |
| `multi_megabyte` | 0.043 | 0.004 | 0.892 | 8490.100 | 26504.241 | 128 | 1.426 |
| `mixed` | 0.022 | 241.923 | 1.452 | 4078.273 | 12115.634 | 126 | 0.732 |

Small load is cadence-dominated. Large-load queue/cadence waits are negligible;
its own serialized writing and FULL commit dominate after admission. Mixed small
admissions wait behind earlier large commits: their final mean ordered wait has a
258.01 ms median. Publication itself takes only about 0.05/1.43/0.73 ms total per
small/large/mixed workload. It already happened per committed piece; delaying one
notification until the end of a flush was not the cause.

The earlier split produced 1,664 FULL commits for 128 large admissions. The final
implementation uses 128, reuses admission's checked sizes instead of serializing
payloads again, and prevents a small prefix from joining a later large admission.
Batch-build totals fall from a baseline median 9,070.21 ms to 0.89 ms for the large
load and from 3,759.83 ms to 1.45 ms for mixed. The native physical-column fix handles
coalesced variable-size rows within the existing chunk bounds in the same transaction.
Larger internal state captures retain bounded pieces and durable-prefix retry.
Controlled proofs cover those mechanisms without assertions about real time.

**The owner accepted these measured lag values on 2026-11-13.** The quietest
observed final p95 values are 177.80 ms large and 172.86 ms mixed; these are observed
floors on this run set, not universal hardware limits. Final single FULL commits
stall as long as 1,148.54 ms (large) and 466.78 ms (mixed), while writing maxima are
reported separately. The earlier split baseline includes a 17,495.83 ms commit
stall. Absolute before/after latency therefore includes substantial shared-machine
I/O variation; reduced commit count and eliminated serialization are measured
mechanism changes, not a claim that all variability came from batching or builds.
No durability setting, budget, deadline or workload was weakened.

An accepted-data snapshot could retain a coherent accepted read image independently
of writer I/O and expose newer accepted data promptly. It cannot reduce the physical
acceptance-to-durability floor or label uncommitted data durable. The owner decided
against that read/retention mechanism on 2026-11-13; no such snapshot is built. Native Linux/hosted
proof remains pending. Raw complete runs, intermediate failed/partial experiments
and compiler evidence are retained under `target/benchmarks/storage/`; incomplete
experiments and the discarded instrumentation run are excluded from these tables.
Nightly exports one median per case plus p95 spread, repetitions and stage counters.

The upload owner now verifies a replayed accepted prefix under the file's serialized
command ownership, including short backend reads. Matching bytes under a different
request identifier return the unchanged producer state; different bytes retain a
rejected receipt and never alter the length or hash. Focused proof is
`replayed_file_bytes_preserve_the_accepted_prefix::replayed_file_bytes_preserve_the_accepted_prefix`,
including fencing, restart and terminal digest integrity. This closes only the
file-byte replay part of task 3.4; the task stays open for the other Append families.

### Native Storage Migration

The 52 storage and eight recovery hooks are implemented with no old-name aliases;
projection definition/status/advancement pairs use optional retained revision/frontier
arguments and capability flags have one owner. Stored encodings and atomic publication
are unchanged. Focused Windows proof covers projection rebuild/restart, finished-file
and mutation replay, and recovery checkpoint/journal restoration.

Required call-site edits outside the storage split are confined to
`crates/harmos-runtime/tests/{streams/revisions.rs,support/storage_fixture.rs,system/runtime.rs}`.
The required Rust check also needed the example guest's one-attempt policy literal
migrated to `retries: None` in `examples/schematic/guest/src/lib.rs`, preserving its
budgets. The required formatter wrapped one Python conformance-fixture policy call.
Protocol Rust, example descriptor and Python storage outputs were regenerated from
`storage.proto` to expose the three capability flags; generated files were not edited by hand.

### Transfer Default (Amended by the Owner on 2026-11-13)

Transfers remain unary. `StorageSettings.transfer` holds the shared generated
`harmos.storage.v1.StorageTransferSettings`; its sole setting is `chunk_bytes`, defaulting
to 4 MiB for both AppendFileBytes and GetFileBytes. A proto2 field default records
that value once for all generated languages. `TransferFraming.Bytes` is the protocol
framing allowance, not a second consumer chunk setting. The owner separately raised
the default message ceiling to 8 MiB on 2026-11-13. Generated
`harmos.storage.v1.MessageSettings.message_bytes` declares it once; all four SDKs,
the listener, plugin carrier and Wasm adapter read that generated default.
`required_message_bytes` computes the chunk-plus-framing minimum independently;
row-page sizing remains independent. The default
listener decoded-retention budget is six framed messages, accommodating its existing
conservative allocation accounting, original request and attempt clone without
changing that accounting. Consumers retain ownership of aggregate admission budgets.

Default listeners, SDK connections, plugin carriers and the Wasm command adapter
use the separate message default. A ceiling below the chunk-plus-framing minimum refuses at
construction/startup naming `StorageSettings.transfer.chunk_bytes` and the conflicting
setting. SDK-created connections own their transport settings; externally injected
channels/invokers retain their caller's transport ownership. Large default-chunk echo
journeys and unary storage journeys cover real message framing rather than constants.

### SDK and Application Content Completion

The owner authorizes four local building commits under the existing combined marker.
The inventory and proof are tracked as C.1–C.5 in tasks.md; no lifecycle or delivery
command is part of this work. The README entry points remain accurate for the message
default; affected configuration details live in the architecture and storage guides.

The conformance fixture becomes a public SDK definition, with a thin executable
caller and packaged generated fixture messages. Each language's isolated package
journey must construct it using only the installed SDK and run the reference host.
The public shapes are `harmos::conformance::plugin() -> harmos::Plugin`,
Python `harmos.conformance.plugin() -> Plugin`, C#
`new Harmos.Conformance.Fixture().Build() -> Harmos.Sdk.Plugin`, and C++
`harmos::conformance::plugin() -> harmos::Plugin` from `harmos/conformance.hpp`.
One `proto/harmos/conformance/v1/fixture.proto` generates all fixture bindings.
Rust ships its generated source/descriptors in the crate; Python ships generated
modules in the wheel; C# compiles messages into the SDK assembly; C++ builds them
unconditionally into `Harmos::Cpp`, including with tests disabled. C++ Build stages
the source distribution with `sdk/cpp` and `proto`; no new release asset is added.
The four package stories use actual Cargo archives, a wheel, a NuGet package and
that source distribution, with no workspace product sources in the consumer.
The generated C++ resource methods take `grpc::ClientContext&` supplied by the caller;
Watch readers borrow that same caller-owned context until the reader finishes.
The matching Rust request, Python call options and C# CallOptions retain per-call
metadata, deadlines and cancellation. No per-operation forwarding code is authored.

Application content registers with the storage read owner through one shared
content-source trait: resolve an immutable reference under the inherited opaque
authorization scope to length, SHA-256 and an explicitly expiring bounded lease,
then read ranges through that lease. The same trait replaces diagnostics' range
callback and the bundle reader's independent content trait. The host application
retains its bytes and metadata; library saved references retain their existing path.

#### Application Content Public Shape

```rust
storage.register_content_source(Arc::new(source))?;
pub trait ContentSource: Send + Sync + 'static {
    fn resolve<'a>(&'a self, reference: &'a str, scope: Option<&'a str>)
        -> StorageFuture<'a, ContentLease>;
    fn read<'a>(&'a self, lease: &'a ContentLease, offset: u64, maximum: NonZeroUsize)
        -> StorageFuture<'a, Vec<u8>>;
}
pub struct ContentLease {
    pub length: u64,
    pub sha256: [u8; 32],
    pub expires_at: SystemTime,
    pub max_bytes: NonZeroUsize,
    pub token: Vec<u8>,
}
let read = storage.open_content(&reference, caller_scope, None).await?;
let range = read.get_bytes(offset, maximum).await?;
```

There is one application resolver per StorageManager, retained until shutdown;
registration refuses replacement. Library-history references use saved storage.
An application supplies a distinct history and an immutable metadata identifier;
its resolver receives that identifier and the inherited opaque invocation scope,
returns authoritative length/digest and a bounded expiring lease, and owns the token.
The library refuses reference/metadata disagreement, oversized tokens/ranges, empty
or oversized provider reads and expired leases, with GetFileBytes named in errors.
Ordered nonempty short reads are accumulated within the requested range.
No application bytes or metadata are written into library persistence.

The existing SavedSnapshots observation mechanism retains the ContentRead handle
under a returned ReadCut, charging its existing capacity. The observation plan binds
the complete reference and caller scope. Continuations reuse that exact lease;
scope changes, expired/evicted leases and restart refuse rather than resolving anew.
This is one retained-cut owner, not another cache or manager. Library saved-reference
reads retain their existing durability/provenance behavior and grants remain separate.

Opaque invocation Scope moves to the lower operations context owner, with the runtime
re-export preserved, so storage handlers read it without a reverse dependency.
Diagnostics uses the same ContentSource/ContentRead contract for each bounded archive
range; required consumer `StatusSettings.read_timeout` bounds that call's lease.
The archive itself still lives until replaced, so this does not introduce archive TTL.
Plugin installation takes the StorageManager directly and opens one content read;
the former bundle-only ContentSource, ContentRead alias and PublishedContent adapter
are removed with their constructor parameter and all callers.

The cross-face expiry journey also requires failure-chain lookup to traverse a
boxed remote Failure, preserving the action already carried in its causal record.
The existing origin/carrier round-trip test covers this missing traversal.

#### Consumer Adoption from 3c91c148

Regenerate protocol/SDK bindings and adopt the generated 8 MiB message default;
retain 4 MiB transfer chunks. Configure injected transports and caller-owned byte
budgets for the selected ceiling. Import Python's fixture from `harmos.conformance`,
and use the public constructors above in layered SDK conformance programs.
Every C++ root call now takes its caller's ClientContext first. Watch returns the
reader directly; retain the context, call TryCancel when abandoning observation,
and Finish the reader before releasing it. Remove the old wrapper access.
Construct the plugin manager with `(storage, settings)` and remove PublishedContent;
supply positive `StatusSettings.read_timeout`. Hosts serving existing content register
ContentSource and retain returned leases/read cuts. Persisted library content needs
no migration, and no second copy of application content is created.

### Unary Transfer Migration

Delete native ReadFile/WriteFile operation declarations and handlers, their two proto
requests and generated bindings, their streaming-only stories and fixture branches,
and the streaming benchmark cases. Keep native file readers and production APIs,
WatchFileBytes/WatchFileProduction, their FileBatch/FileCompletion payloads and all
replacement/truncation/fencing/terminal behavior. The benchmark measures only unary
client/plugin uploads and downloads, with integrity and publication unchanged.

| Carrier and Direction | 64 KiB Unary MiB/s | 1 MiB Unary MiB/s | 3 MiB Unary MiB/s | Streaming Comparison MiB/s |
| --- | ---: | ---: | ---: | --- |
| Client Upload | 62 | 182 | 189 | 300–390 |
| Client Download | 125 | 190 | 211 | Measured baseline retained below |
| Plugin Upload | 42 | 70 | 95 | No plugin input stream |
| Plugin Download | 60 | 87 | 137 | Measured baseline retained below |

The owner reviewed these 30 MiB reference-memory, three-run medians and chose
4 MiB unary chunks. The earlier 64 KiB table below remains measurement history,
not permission to retain streaming implementations. The new-default run is recorded below.

### Unary Transfer Implementation

Both face services own seven independent wrappers in `proto/harmos/{v1,context/v1}/storage.proto`.
Shared domain payloads retain their owner in `harmos.storage.v1`, now declared in
`storage_types.proto`; they do not import another face's wrappers. The storage operation
owner declares client methods and context-only methods with shared handlers in
`crates/harmos-operations/src/storage/`. Native operation identifiers mechanically use
`<package>.Storage/<Method>`, matching the core identity grammar and gRPC full method.
The shared Identifier alphabet accepts that canonical separator. Reserved declarations
are checked against their authoritative face descriptors before host registration.

`StorageManager::get_file_bytes` reads bounded immutable ranges at saved progress.
`publish_file_production_once` adds explicit publication length/digest validation using
the existing production, receipt and durability owners. Its public operation waits for
the reference/receipt barrier. Existing native streaming publication fingerprints and
stored encodings remain unchanged. Status excludes in-flight appends and observes the
committed writer generation without creating a writer. Session status and completion
carry retained prefix progress without inventing a primary-history position.

Focused proof: `storage::unary_transfers::resume_fence_publish_and_range_across_faces`
passes through a real socket client and a real plugin child carrier, including fenced
resume, byte replay/conflict, grants, unsupported sessions, integrity, abandonment and
exact ranges. Issued-position retention and complete paging/Watch remain their separate
tasks. Streaming upload/download are deleted following the owner's comparison.

Coupling edits: protobuf generation/reflection includes both storage faces and the
shared payload file move (including import-only edits and generated consumers). Native
host registration uses factory validation, matching the core worker's existing fix,
and child fingerprinting reads native protobuf descriptors without requiring a client
gRPC projection. README links the new transfer API; no other runtime subsystem changes.

### Recoverable Upload Progress

Status exposes admitted bytes separately from the last acknowledged production
recovery checkpoint. Pending and completed but uncheckpointed appends never become
`durable_length`; reopening restores that length from the persisted checkpoint.
Publication still requires its separate reference/receipt barrier. A lost checkpoint
reply retains the prior acknowledged length until the same checkpoint is retried.
The cross-face transfer story controls a blocked append and a failed checkpoint reply,
then checks exact ranges, corruption refusal and the complete encoded response ceiling.
Structured transient failures retain retry classification, and replay payload/prefix
conflicts retain the conflict classification. The existing lost-reply/cancellation
receipt story passes without changing its retry budget or durability guarantees.

### Windows Unary Transfer Measurements

Measured on 2026-11-13 with the source-complete storage carrier paths, the existing
reference memory backend and real client/plugin sockets. Each of three repetitions
transfers the same deterministic 30 MiB file with the existing 64 KiB chunk/range
configuration. Session creation and publication are timed. Download timing includes
incremental hashing on both carriers; setup and digest comparisons are outside it.
Unary downloads reuse retained saved images rather than copying the whole file for
each range. Full-frame bounds result in 481 ranged calls; uploads use 482 session/
Append/publication calls. Streaming calls consume the same bytes in one invocation.
No consumer budget, deadline or workload size changed.

| Carrier and Direction | Unary MiB/s, Median (Minimum–Maximum) | Retained Streaming MiB/s, Median (Minimum–Maximum) |
| --- | ---: | ---: |
| Client Upload | 56.89 (51.53–61.54) | 394.02 (223.28–516.48) |
| Client Download | 100.94 (94.65–137.97) | 355.03 (178.85–460.33) |
| Plugin Upload | 38.54 (38.28–39.25) | Unavailable: child input streams are refused |
| Plugin Download | 52.91 (52.71–54.74) | 94.78 (75.53–122.46) |

The historical plugin streaming download invoked the then-retained ReadFile handler through output
child calls with explicit demand, counts/hashes every item and verifies completion.
It returns a typed byte count/digest instead of materializing the file in a unary
parent response. It is not a native-call substitute. Streaming upload has no existing
plugin-carrier input mechanism; runtime admission rejects child input streams, so
no throughput is invented for it. These are transport/reference-provider results,
not durable-disk throughput or hosted Windows/Linux proof. This historical streaming
implementation was deleted by the owner's 2026-11-13 amendment. Raw output/compiler evidence remains in `target/benchmarks/storage/`, and
Nightly exports the same case's medians.

### Face Generator Integration

Merged core checkpoint `8b5ed431` and regenerated independent storage face bindings.
The transfer fixture now serves `harmos.plugin.v1.Instance`; the focused cross-face
resume story passes. Coupling repairs update SDK Wasm host imports to shared/context
owners, fix the runtime guest host import, remove a duplicated generator input scan,
and retain storage package flattening. No compatibility module or carrier alias remains.

### Definition Compare and Exchange

Index and projection definitions use their provider-owned revision. Native Set takes
`expected: Option<u64>`: `None` explicitly creates an absent definition; `Some(revision)`
compares the current revision inside the same provider transaction or catalog mutex.
The wire uses the Get revision as eight opaque big-endian bytes and a mutually exclusive
create-if-absent flag. Validation covers the complete definition before replacement;
rebuild remains explicit. Even an unchanged candidate with a stale revision conflicts,
naming the Set action and “get again and retry”. The two faces share these handlers,
while each declaration and grant retains its own identity. Retired RegisterIndex and
RegisterProjection operations, request types and SDK bindings are removed. Native
bench/provider consumers now supply expected revisions. Object/schema mirrors and
issued-cut retention remain separate open work.

### Issued Storage Cut Lifetime

Storage read cuts retain existing durable provider snapshots through the existing
`SavedSnapshots` owner. Omitted `at` immediately selects the current saved position,
without requesting persistence. A supplied complete cut must match a retained tuple
that this owner returned. The existing consumer-supplied saved-read capacity bounds
issuance: oldest idle automatic cuts may expire; explicit CreateSnapshot leases stay
pinned until DeleteSnapshot or storage retirement. Active readers retain their image.
Failure/cancellation of one page does not discard an explicit lease. Capacity refusal
is explicit when pinned or active readers occupy every slot.

ListRows tokens are HMAC-authenticated with an owner-local key and bind the retained
image, normalized source/transforms/order and query progress. Page size may change
within consumer bounds. The latest continuation request/result is cached for replay with identical bounds;
consuming its next token expires older requests. Retained query plans advance one
bounded source/output step per call, including empty progress pages. Idle plans may
expire under the existing retained-read capacity. They release active execution
between calls and bind each resumed step to the current invocation context. Positions and definition revisions are retained as one exact composite;
callers cannot synthesize another combination. There is no arbitrary historical
fallback. A provider unable to retain the exact requested revision refuses rather
than selecting a newer definition or frontier. A selection involving a different
storage history is unavailable on this owner.

Read-lease issuance and signing keys are not recovered across manager retirement/restart.
Retained mutation receipts may reissue their exact position after the primary saved
frontier proves durability, including recovered receipts. Pending positions refuse
without requesting persistence, and expired receipts or lost provider retention
refuse explicitly. Receipt identity lifetime and a separately retained read lease
have independent expiry: losing an idempotency key does not silently reclaim a
still-pinned durable image.
Expired/foreign/changed leases or tokens require restarting the read with no `at`.
A still-readable durable position can later be issued again as the same value;
this does not revive its old opaque page token. Provider/recovery internals retain
their explicit durable-cut APIs; public client/context continuations use this issuer.
There is no snapshot of accepted data. The file-range handler now reuses the same
retained durable snapshot for all ranges instead of opening/copying it per call.

### Append Receipt Completion and Capacity Retirement

The three storage Append families admit one bounded immutable batch and return
its fixed request identity and accepted position immediately. Replaying the same
normalized input observes the same admission; durable completion advances only
after both the primary data and the replay ledger have acknowledged persistence.
The storage worker owns this second stage after caller loss. No read requests it.
Native `wait_mutation_durable` observes that owner without scheduling a flush;
wire callers resend their original Append. An unacknowledged ledger response
cannot populate `durable_at`, even if the data frontier already covers the batch.

Unsaved cursor receipts are retained until this stage settles. At capacity,
evicting a completed replay entry rotates the issuance window: retained identities
continue to replay, while an absent identity from the retired window refuses
rather than becoming fresh work. New mutations obtain the current window from
Capabilities. Restart restores retained acknowledged receipts; missing identities
from a previous manager window refuse. This preserves explicit expiry and the
consumer's bounded receipt capacity without an unbounded tombstone collection.

### Native List Continuation and Metadata Reads

ListColumnRows, ListIndexEntries and ListProjectionRows use the common PageRequest
and optional issued ReadCut. Their selectors carry physical work/ordering, while
page size and response bytes belong to PageRequest. The owner seals the retained
read identity, selector digest and native cursor with an owner-local HMAC. Tokens
are stateless within that retained image and may replay/change page geometry;
ListRows separately retains its query execution state and latest-result replay.
Both mechanisms expire explicitly on owner retirement, lost retention or restart.
No native cursor or retained lease token is an alternate public paging input.

Index composite cuts select a definition revision and the provider validates it
inside its scan, including the first page. A rebuild cannot silently relabel a
page with a newer definition. GetIndexDefinition refuses a supplied unavailable
revision; GetProjectionDefinition selects its retained immutable revision.
Projection lists/status select the exact issued revision/frontier or the newest
readable materialization when at is omitted. Generic primary-only cuts can be
augmented with the selected participating resource revision by the read owner;
returned composite cuts bind subsequent pages. Caller-authored combinations are
never accepted as issued cuts.

A short physical column page is not evidence of end-of-source. Filtered empty
pages retain physical progress; empty source or the selected frontier proves
completion. Native output packing includes IPC schema bytes as well as retained
buffers; the full wire envelope is checked against both response ceilings.
Numeric column bounds compare integer endpoints without converting stored
integers to floating point.

### Storage Operational Policy

`StorageSettings.policy` owns backend I/O/recovery deadlines, shared replay-safe
retry policy, production lifetime and receipt retention. Previous durations and
attempt counts remain its defaults. Consumers explicitly choose maximum retained
productions and mutation receipts in request settings, alongside existing admission
and response budgets. A child-call policy override is invocation-local; it does
not mutate either the operation declaration or storage configuration. Controlled
clock/provider stories cover exhaustion, explicit repair, abandonment, receipt
expiry and the next call inheriting its unchanged declaration.

### Durable Row Watch Contract

WatchRows uses the ListRows source selector for records, samples and object
revisions (Latest object selection refuses). Opening resolves optional `at` using
the issued-cut owner and immediately emits that boundary, with no implicit initial
list. Subsequent entries are exclusive of it, ordered by storage position and
released only after that position is durable. A ListRows snapshot supplies initial
state when required. Subscription retention is registered before the opening
response; commits during opening cannot disappear between the boundary and tail.

An event reports an issued durable cut through its examined prefix; progress
advances across unmatched committed entries. Accepted followers retain their
separate native contract. The existing bounded queue owns watch retention: when
pressure retires a slow observer, WatchRows emits resync_required with its last
issued boundary and terminates. The caller lists a fresh durable image and opens
a new watch from its returned cut. Expired/foreign input cuts refuse before the
opening boundary. Close or restart retires observers; reconnect uses only a still
retained issued cut, never an arbitrary earlier sequence or silent latest fallback.

### External File and Directory Watches

The three per-face external Watch methods share native file/production/directory
owners. Stored immutable content requires durable publication. Local file watches
retain source/start, explicit replacement/truncation and finite byte-count
completion; they do not claim local bytes are a saved primary history. Production
observation retains checkpointed prefix, offset, fencing and terminal progress;
observation loss never publishes, abandons or deletes content. Directory
subscription precedes enumeration and preserves initial boundary and explicit
rescan-required uncertainty. Native selection, cancellation and lifetimes remain
owned by those readers. Whole wire envelopes, including nested event lengths,
count toward consumer response limits without changing the limits themselves.

### Filesystem Metadata and Retained Image Identity

GetFileHash, CaptureFile and ListDirectoryEntries now share handlers across the
independently declared faces. A local hash, production status or directory listing
retains only bounded result metadata under the existing issued-cut owner. Explicit
`at` repeats that observation, not a newer filesystem image. Directory enumeration
is ordered and bounded but not an atomic filesystem snapshot. Whole response
framing counts against page limits. A lease has its own release handle while
sharing the retained image identity; pinning therefore preserves already issued
continuations through automatic cache eviction. Restart changes issuance ownership
and refuses old cuts/tokens even if persisted primary positions are unchanged.
No accepted-payload snapshot is introduced. Capture retains unknown replay safety,
its existing consistency modes and completed durable publication; without a caller
mutation ID it does not promise replay-key deduplication.

### Native Schema Compare/Exchange

The four native hooks are Protected Info/GetSchema/SetSchema/DeleteFile and have
separate service-qualified wrapper pairs without a wire service. GetSchema uses
the issued saved-cut owner and returns a complete candidate with its exact stored
position as an opaque revision. SetSchema atomically compares an expected revision
or explicit absence, validates the whole graph, and refuses a changed graph under
an existing immutable key. New versions use a new key. The existing AppendSchemas
provider hook accepts either an ordered append or an atomic schema exchange;
there is no additional backend hook. SQLite commits the candidate/refusal, saved
position and replay outcome together under WAL/FULL. Admitted work and its receipt
remain manager-owned after caller loss. The stored schema bytes are unchanged.
DeleteFile only gains its Protected wrapper; retained-reader reclamation remains
the separate undecided task 3.10.


### Consumer Adoption After Storage Convergence

A consumer pinned at `af739a4e` must regenerate its contracts and update requirements
and fingerprints together. Replace native HashFile/CaptureFile/directory/follow
requests with the face-specific Storage GetFileHash/CaptureFile/ListDirectoryEntries
and WatchFileBytes/WatchFileProduction/WatchDirectoryEntries wrappers; unwrap the
payload and retain resolved cuts. WatchRows now selects records, samples or object
revisions, opens at an issued durable boundary and reports explicit resynchronization;
list first when initial state is needed. Native storage hooks use Protected
Info/GetSchema/SetSchema/DeleteFile wrappers; schema Set carries a whole candidate,
expected revision or explicit absence and mutation identity. Consumers supply
StorageSettings policy and request admission/retention fields, and preserve issued
cuts/page tokens across bounded native lists. Existing unary upload/ranged bundle
and diagnostics readers, plugin manager/access/status composition and core wire
identity contracts remain available. On-disk schema bytes and durability guarantees
are unchanged. Native ReadFile/WriteFile and their comparison cases are deleted by the owner's
2026-11-13 amendment.

### Default 4 MiB Unary Transfer Measurements

Rerun on Windows on 2026-11-13 with the reference memory backend, real sockets,
30 MiB files and three repetitions per face at the storage-owned default. Uploads
used ten session/append/publication calls; downloads used eight ranged calls.
Integrity, full-chunk admission and publication remain included. This session's
conformance compilation finished before the timed run; other workstation activity
was not controlled. These are measurements, not a throughput promise.

| Carrier and Direction | 4 MiB Unary MiB/s, Median (Minimum–Maximum) |
| --- | ---: |
| Client Upload | 154.35 (143.87–203.50) |
| Client Download | 191.24 (164.86–214.80) |
| Plugin Upload | 98.63 (73.60–98.67) |
| Plugin Download | 112.48 (90.92–117.00) |

Raw output and compiler evidence remain under `target/benchmarks/storage/`.
