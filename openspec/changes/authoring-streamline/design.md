# Authoring Streamline Design

## Status And Scope

Active implementation contract. All six writers have handed off; the sole integration agent owns all A-F paths on feat/authoring-streamline. Source closure and validation remain pending.

Development clean break: remove request-as-operation authoring, mandatory Resources associations, manual operation versions, obsolete registration verbs and encoding aliases; migrate all repository callers. Required qualified policy re-exports are aliases to sole definitions, not legacy adapters. All protocol, envelope, journal and schema versions are 1. Old data and peers are unsupported: remove compatibility, migration and old-format detection paths; retain current-format validation, fingerprint and generation fencing. Structural fingerprints cannot detect semantic-only changes.

## Models And Identity

`model/operations/policy.rs` owns OperationPolicy { timeout, attempt_timeout, retries }, RetryPolicy, Backoff, Jitter and pure validation. `model/services/policy.rs` owns ServicePolicy { stop_timeout: Duration, restart: RestartPolicy } and RestartPolicy. Remove `operations/common/policy.rs` after migration. Rename only RestartPolicy::OnTransient to OnFailure; retain request retry classification/replay safety. Observers use ServicePolicy and the resident supervisor, including health-credit replenishment.

OperationDeclaration is portable: Request/Response: Portable, Input/Output: existing Direction, with ID, POLICY, REPLAY, VISIBILITY and GRPC retaining their meanings. Its definition() returns the shared OperationDefinition (existing Definition metadata plus fingerprint, no manual version). The struct macro never serializes resource fields or emits payload implementations. Runtime associated Request/Response must match the declaration, and endpoint item types must match declared streams. No generated per-operation definition struct.

Fingerprint and Digest are distinct newtypes over [u8; 32]; binary encoding is 32 raw bytes and text is 64 lowercase hex digits. Hash with SHA-256 and domain prefixes `harmos.schema.v1\0`, `harmos.content.v1\0`, `harmos.operation.v1\0` (UTF-8 plus one NUL). No Rust Hash, TypeId, pointers, module names or serde formatting enters identity.

Canonical schema: traverse the existing finite schema graph depth-first from its root, in declaration/wire field order. Assign zero-based node indices on first encounter before visiting children; repeated/recursive edges reference that index. Encode node count (u32 LE), then each node's field count (u32 LE), then fields: name (u32 LE byte length + exact UTF-8), tag (u32 LE), kind byte (Bool/U32/U64/I32/I64/F32/F64/String/Bytes/Message = 0..9), cardinality byte (Required/Optional/Repeated/Variant = 0..3), and a u32 LE target index only for Message. Root is zero; reject unreachable nodes. Field order matters for positional encoding. Reindexing graph storage leaves the result unchanged; declared graph sharing is retained, so generators must preserve node identity.

Schema fingerprint = SHA256(schema prefix + length-prefixed codec ID + canonical schema). Content digest = SHA256(content prefix + schema fingerprint + canonical encoded value). Operation fingerprint = SHA256(operation prefix + length-prefixed authored local ID + request fingerprint + response fingerprint + input presence/optional fingerprint + output presence/optional fingerprint). Presence is one byte 0/1; strings use u32 LE byte lengths. Qualified routing identity is checked separately. Host ownership, generations, gRPC routing names, policy and lanes are excluded.

Default codec ID `harmos.postcard.v1` fixes existing field order, shortest varints, enum indices, sequence lengths, UTF-8 and little-endian IEEE-754 bits. Preserve signed zero and every NaN payload; no float normalization or Hash/Eq requirement. Custom Message implementations supply CODEC, canonical encode/decode and decoding_memory, plus MemorySize and Described; omit #[message]. Reject unsupported codecs at carrier registration. Golden vectors cover nested/recursive schemas, streams, NaNs and signed zero across supported language generators.

Message exposes encode() -> Vec<u8>, encode_into(&mut Vec<u8>), encoded_size() -> usize and decode(&[u8]) -> Result<Self, MessageError>; Portable adds fingerprint() and digest(). MemorySize remains capacity-aware and saturating. encode_into appends and reuses capacity. Remove encode_to_vec/encoded_len and duplicate Storable::encoded_size. Preserve decoding_memory preflight and caller admission budgets. Ordinary derives remain optional; explicit Debug derive or skip_debug suppresses generated Debug.

## Construction And Invocation

OperationContext is cloneable, SDK-owned and carrier-local. dependency<T: Send + Sync + 'static>() -> Result<Arc<T>, DependencyError> reads an immutable Dependencies snapshot. Explicit insert<T>(&mut self, Arc<T>) rejects duplicate types; a missing dependency reports its type name during initialization. Product newtypes distinguish multiple resources of one underlying type. No automatic construction, global lookup or cross-process Arc/TypeId serialization. Each carrier supplies local dependencies, including typed host/supervisor accessors; those accessors retain authorization checks.

Operation: Send + Sized + 'static has Request, Response, Error and Endpoints. initialize(OperationContext) constructs one Self per admitted invocation. run(&mut self, OperationContext, &mut Request, &mut Endpoints) returns Response<ResponseType>; futures are Send. Retain request classify(error) and transferred() semantics. Invocations never share Self; retries reuse the invocation's Self, request and endpoints, preserving existing transfer/replay guards. Stop and cleanup run once on every initialized instance, including cancellation, and default to success. Failed initialization owns cleanup of partial resources. Admission limits bound concurrency; shared Arc dependencies synchronize their own mutable resources, independently of lanes.

Service is separate: initialize(context), run(&mut self, context), stop and cleanup; Error but no request, response, classify or Operation inheritance. Eligible run/initialization failures spend restart credit. Await cleanup, discard Self and re-initialize from the same dependency snapshot before replacement execution. Explicit shutdown/cancellation never restart. Preserve health replenishment, stop deadlines and observer replay/batch obligations. Cleanup failure retains ownership and blocks replacement until settled.

Request inference is opt-in: #[operation(..., infer_request)] emits `impl RequestFor for ReadRequest { type Operation = Read; }`. Exactly one declaration in the request's owning crate opts in; no blanket inference or registry search by TypeId. Reused/foreign requests use runtime.operation_for::<Read>(request); other operations omit infer_request. Both forms use portable declarations and the same routing.

Invocation<O, L> owns the request, optional retry/lane overrides and stream endpoints. Construction is inert. start/run consume it and validate authority, complete contract, retry/replay and lane before atomically snapshotting policy/provider generation at admission. run is start followed by join, preserving Response<T> ownership. Keep status/cancel/detach/cancellation-on-drop. endpoints(E) supplies streams before start; retain bounded backpressure, item accounting and transfer fences.

## Registration And Lanes

Keep Runtime<A>; add Application::Lanes: Lane (closed Copy + Eq + Send + Sync enum), retaining the existing lane macro and consumer capacity settings. Catalog::catalog() yields portable definitions in both builds; runtime-gated Catalog::registration(Dependencies) yields SDK RegistrationBatch factories. Individual declarations expose registration by the same machinery. Runtime's RegistrationExt::lane(L) converts that batch to Registration<L>; defaults::<L>() selects host defaults, and default_lane_for<O>(L) overrides a member. No runtime enum enters a portable declaration. Missing placement uses the host enum's declared default; unknown carrier lane tokens are rejected before admission.

register accepts typed Registration<L>; managers own authoritative entries and the facade coordinates them. PluginScope carries an unforgeable host-assigned owner; root registration uses host. Qualified IDs are owner + "." + local ID. Each segment matches [a-z][a-z0-9_]*; no escaping, embedded dots or caller prefixes. Reserve host and harmos for the host. Migrate dotted local declarations. Advertisements contain local IDs; routing envelopes contain already-qualified IDs, validated without adding another prefix.

Entries carry owner, qualified ID, contract fingerprint and monotonically increasing u64 generation; overflow rejects replacement. Validate the entire batch (ownership, duplicates, contracts, declared placement and registration dependencies) before publishing any member; failure releases reservations. Factories initialize lazily, so publication does not promise device readiness. unregister(&QualifiedId) withdraws admission, cancels active work, awaits stop/cleanup and releases resources. Failure retains the closing entry/provider owner; another unregister resumes settlement. Other entries remain available. Re-registration requires settled withdrawal and allocates a fresh generation; stale frames cannot affect it. Plugin unload withdraws all owned contributions, including dynamic entries, before releasing code.

Deferred: automatic reload, readiness handover and rollback; explicit unregister then register supplies the needed consumer workflow tonight.
Deferred: new host-plugin protocol architecture; migrate existing advertisements/envelopes for identity and generations while preserving current transport/lifecycle guarantees.
Deferred: canonical company skills and generated copies outside this repository; this worktree-only assignment cannot edit those owners. Repository Harmos skills, examples and scaffolding migrate now; external alignment is a follow-up.

## Final Consumer API

Operation file `gauge-operations/src/temperature/read.rs`; Device/Error come from a lower-level resource crate and Error converts DependencyError:
```rust
#[cfg(feature = "runtime")]
use {device::{Device, Error}, harmos::{Operation, OperationContext, Response}, std::sync::Arc};
#[harmos::message]
#[derive(Clone, PartialEq, Eq)]
pub struct ReadRequest { pub channel: u32 }
#[harmos::message]
#[derive(Clone, PartialEq)]
pub struct ReadResponse { pub value: f64 }
#[harmos::operation(id = "read", request = ReadRequest, response = ReadResponse,
    policy = crate::POLICY, infer_request)]
pub struct Read {
    #[cfg(feature = "runtime")]
    device: Arc<Device>,
}
#[cfg(feature = "runtime")]
impl Operation for Read {
    type Request = ReadRequest;
    type Response = ReadResponse;
    type Error = Error;
    type Endpoints = ();
    async fn initialize(context: OperationContext) -> Result<Self, Error> {
        Ok(Self { device: context.dependency::<Device>()? })
    }
    async fn run(&mut self, _: OperationContext, request: &mut ReadRequest,
        _: &mut ()) -> Result<Response<ReadResponse>, Error> {
        Ok(ReadResponse { value: self.device.read(request.channel).await? }.into())
    }
}
```

Catalog in the operations crate, followed by host composition; Application::Lanes is Lane:
```rust
#[harmos::catalog]
pub enum Operations { Read(temperature::read::Read) }
let definitions = Operations::catalog(); // Available Without runtime
let mut dependencies = harmos::Dependencies::new();
dependencies.insert(device)?;
use harmos::RegistrationExt;
runtime.register(Operations::registration(dependencies).lane(Lane::Interactive)).await?;
let reading = runtime.operation(ReadRequest { channel: 3 }).retry(retry).run().await?;
let pending = runtime.operation_for::<Read>(ReadRequest { channel: 3 }).start().await?;
let reading = pending.join().await?;
runtime.unregister(&harmos::QualifiedId::parse("host.read")?).await?;
```

Facade defaults become domain-only; optional runtime enables SDK/runtime exports, and existing execution features imply runtime. Consumer operation crates use default-features = false and runtime = ["harmos/runtime", "dep:device"]. Feature unification is acknowledged, not hard isolation. Keep shared models/transactions in product-domain and resource-owning services in product-runtime. Built-ins use `harmos-operations/src/{host,lifecycle,storage,streams}/`: one file per operation owns messages, the operation macro struct and its runtime-gated implementation. Concern catalogs and Builtins are exported from lib.rs. The crate depends on harmos-domain and optionally harmos-sdk/harmos-storage, never harmos-runtime. Runtime inserts SDK-owned state access handles through OperationContext dependencies and consumes harmos-operations with runtime enabled. The facade re-exports the operations crate. Remove the old domain declarations/common directory and split runtime implementations without shims. Built-in types use the operations crate directly or the facade's builtins namespace; remove old runtime concern/type forwarding exports and the redundant lifecycle catalog wrapper.

Carrier kits select the operations crate's lightweight `lifecycle` feature for SDK plugin controls. Its full `runtime` feature includes this subset and all host/storage built-ins. The facade enables native built-in execution through harmos-runtime; its Wasm SDK surface does not enable host storage. No operation macro option or consumer authoring convention changes.

## Current Format Ownership

The guest WIT package is `harmos:guest@1.0.0`; the native linker binds its current operation-context interface.

Only current v1 protocol, artifact, envelope, journal and persisted payload shapes
are supported. Transaction, record and stream authoring emits schema version 1.
Remove historical shape catalogs, replay callbacks, protobuf storage encoding,
version-only peer checks and schematic journal upgrades. Retain current-format
bounds, integrity, ownership, fingerprint, generation and durability checks.
