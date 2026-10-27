## 1. Implementation

- [x] 1.1 Design Lead: contracts agreed and `stkr specification start authoring-streamline` active on feat/authoring-streamline. Six parallel writer assignments follow; these are ownership reservations, not agents launched by this planning task. All paths are repository-relative and directory ownership is recursive, including manifests, tests, fixtures and deletions. No writer edits outside its listed paths. Dependencies are interface contracts, not permission to edit the producer. Keep all work in this worktree/branch; no external repository or installed-cache edits.
- [ ] 1.2 All Writers: use the signatures below without inventing adapters or alternate APIs; `async fn` in trait listings means `fn -> impl Future<Output = ...> + Send`. E is std::error::Error + Send + Sync + 'static, L is Lane, O is OperationDeclaration, and R is RequestFor. Existing fields/methods not explicitly replaced retain their signatures. Before editing identities or encodings, each writer audits persisted/wire version consumers in its territory and preserves application data. Each writer owns migration of every reference within its territory and reports cross-owner discoveries without editing them. No compilation during parallel authoring; record targeted proof obligations for Integration. Each slice hands off source, removals and affected deterministic tests; only Integration closes items after targeted proof.

### Slice A: Portable Models — Owner `portable_models`

- [x] 1.3 Own `crates/harmos-domain/` exclusively. Move request policy to `src/model/operations/policy.rs`, add `src/model/services/policy.rs`, update model catalogs/re-exports, remove old policy definitions and migrate built-in declarations. Own schema/message/contract identity, qualification value types, portable frames and all domain fixtures.
- [x] 1.4 Provide these contracts at domain root and their owning namespaces; update existing contract constructors/accessors from version to Fingerprint. OperationDefinition replaces Definition everywhere, with fields `id: Identifier, description: String, policy: OperationPolicy, visibility: Visibility, replay: ReplaySafety, fingerprint: Fingerprint`. Input/Output, descriptions, visibility, gRPC and replay defaults remain as today. The declaration no longer implements Portable itself. Grant/cache hooks now inspect the separate request. Invalid static declarations fail macro checks or registration validation, not payload serialization.

```rust
pub struct Fingerprint(pub [u8; 32]);
pub struct Digest(pub [u8; 32]);
pub struct ServicePolicy { pub stop_timeout: Duration, pub restart: RestartPolicy }
// OperationPolicy has timeout, attempt_timeout, retries; RestartPolicy uses OnFailure.
impl ServicePolicy { pub fn validate(self) -> Result<(), PolicyError>; }
pub trait Message: Sized + Send + Sync {
    const CODEC: &'static str;
    fn encode(&self) -> Vec<u8>;
    fn encode_into(&self, bytes: &mut Vec<u8>);
    fn encoded_size(&self) -> usize;
    fn decode(bytes: &[u8]) -> Result<Self, MessageError>;
    fn decoding_memory(bytes: &[u8]) -> Result<usize, MessageError>;
}
pub trait Portable: Message + MemorySize + Described + Send + Sync + 'static {
    fn fingerprint() -> Fingerprint;
    fn digest(&self) -> Digest;
}
pub trait OperationDeclaration: 'static {
    type Request: Portable;
    type Response: Portable;
    type Input: Direction;
    type Output: Direction;
    const ID: &'static str;
    const POLICY: OperationPolicy;
    // DESCRIPTION, VISIBILITY, REPLAY, GRPC retain their existing types/defaults.
    fn definition() -> OperationDefinition;
    fn grant(request: &Self::Request) -> Option<plugin::Grant>;
    fn key(request: &Self::Request) -> Option<CacheKey>;
}
pub trait RequestFor: Portable { type Operation: OperationDeclaration<Request = Self>; }
impl QualifiedId {
    pub fn parse(value: &str) -> Result<Self, IdentityError>;
    pub fn qualify(owner: &str, local: &str) -> Result<Self, IdentityError>;
    pub fn as_str(&self) -> &str;
}
```

- [x] 1.5 Consume B's `#[message]`, `#[operation(...)]` and `#[catalog]` expansion contract; do not introduce a runtime/SDK dependency. Retain the blanket Portable implementation, existing schema validation/admission and payload bytes. Provide fixed identity golden vectors; proof obligations: capacity accounting, decoding refusal, nested/cyclic graph identity, float bits, codec identity, policy credit/backoff and qualified-ID rejection.

### Slice B: Authoring Macros — Owner `authoring_macros`

- [ ] 1.6 Own `crates/harmos-macros/` exclusively. Replace impl/request-based operation expansion with the struct form, add request argument/infer_request, preserve cfg/ordinary derives/custom Debug, and migrate all macro fixtures and tests. Catalog expansion must not depend on executable traits in contract-only builds. Keep current input/output/grant/key/visibility/replay/grpc attributes with hooks taking the separate request; use `policy = OperationPolicy`, not a second retry_policy attribute. Unsupported payload diagnostics name the field/type and correction.
- [ ] 1.7 Provide the following generated inherent methods; register unary and streaming factories from C without duplicating their lifecycle code. Infer_request emits exactly one A::RequestFor impl; absent infer_request emits none. Use the consuming crate's `runtime` cfg for execution-only checks/methods. Preserve the existing lane macro and its capacity/default diagnostics; extend its generated impl to D's Lane bounds.

```rust
impl Read {
    pub fn definition() -> OperationDefinition;
    #[cfg(feature = "runtime")]
    pub fn registration(dependencies: Dependencies) -> RegistrationBatch;
}
impl Operations {
    pub fn catalog() -> Vec<OperationDefinition>;
    #[cfg(feature = "runtime")]
    pub fn registration(dependencies: Dependencies) -> RegistrationBatch;
}
// Read implements A::OperationDeclaration; generated runtime checks require:
// Read: Operation<Request = ReadRequest, Response = ReadResponse>.
```

- [ ] 1.8 Consume A's portable traits/definition, C's `RegistrationBatch::new/operation`, and D's Lane trait; generated execution references go through facade runtime exports, portable references through domain exports. Proof obligations: standalone custom payloads, mismatched payloads/endpoints, duplicate inference, reused request without inference, contract-only catalogs, runtime fields excluded from schemas and feature-enabled compilation.

### Slice C: Execution SDK — Owner `execution_sdk`

- [x] 1.9 Own `crates/harmos-sdk/` exclusively. Implement the dependency snapshot and common Operation/Service traits, factory batches, serving lifecycle and response/instance migration. Remove Resources and carrier-specific Context associated types; migrate grants/cache keys to request references. Preserve request classification, transfer state, streaming ownership, memory accounting and existing Response<T>/Instance semantics. Error defaults/classification remain request-only.
- [x] 1.10 Provide these SDK-root and owning-namespace contracts; OperationDeclaration equality is enforced by factory bounds and macro checks. Dependencies is Clone with immutable shared entries after snapshotting; insertion mutates only the caller's snapshot. OperationContext owns a snapshot plus existing call metadata, and carriers construct it through new. EndpointSet and typed stream endpoint contracts retain their current methods.

```rust
impl Dependencies {
    pub fn new() -> Self;
    pub fn insert<T: Send + Sync + 'static>(&mut self, value: Arc<T>) -> Result<(), DependencyError>;
}
impl OperationContext {
    pub fn new(dependencies: Dependencies, call: CallContext) -> Self;
    pub fn dependency<T: Send + Sync + 'static>(&self) -> Result<Arc<T>, DependencyError>;
}
pub trait Operation: Send + Sized + 'static {
    type Request: Portable;
    type Response: Portable;
    type Error: std::error::Error + Send + Sync + 'static;
    type Endpoints: EndpointSet;
    async fn initialize(context: OperationContext) -> Result<Self, Self::Error>;
    async fn run(&mut self, context: OperationContext, request: &mut Self::Request,
        endpoints: &mut Self::Endpoints) -> Result<Response<Self::Response>, Self::Error>;
    async fn stop(&mut self) -> Result<(), Self::Error>; // Default Ok(())
    async fn cleanup(&mut self) -> Result<(), Self::Error>; // Default Ok(())
    fn classify(error: &Self::Error) -> FailureKind; // Existing Default
    fn transferred(&self) -> bool; // Existing Default
}
pub trait Service: Send + Sized + 'static {
    type Error: std::error::Error + Send + Sync + 'static;
    async fn initialize(context: OperationContext) -> Result<Self, Self::Error>;
    async fn run(&mut self, context: OperationContext) -> Result<(), Self::Error>;
    async fn stop(&mut self) -> Result<(), Self::Error>; // Default Ok(())
    async fn cleanup(&mut self) -> Result<(), Self::Error>; // Default Ok(())
}
impl RegistrationBatch {
    pub fn new(dependencies: Dependencies) -> Self;
    pub fn operation<O>(self) -> Self
    where O: OperationDeclaration + Operation<Request = <O as OperationDeclaration>::Request,
        Response = <O as OperationDeclaration>::Response>;
    pub fn implementation<D, O>(self) -> Self
    where D: OperationDeclaration, O: Operation<Request = D::Request, Response = D::Response>;
    pub fn service<S: Service>(self, local_id: &str, policy: ServicePolicy) -> Self;
}
impl WireTable {
    pub fn new(owner: &str) -> Result<Self, ServingError>; // Validated Carrier-Assigned Owner
    pub fn register(&mut self, batch: RegistrationBatch) -> Result<(), ServingError>;
    pub async fn unregister(&mut self, id: &QualifiedId) -> Result<(), ServingError>;
}
```

- [x] 1.11 Consume A's messages/definitions/policies and qualified frames. The implementation<D, O> factory binds internal built-in declarations to runtime-owned implementations without a domain-to-SDK dependency; operation<O> is the same factory with D = O. WireTable drops its old context generic; it remains the existing serving table, not a second registry. Its constructor validates the owner segment supplied by the carrier after host assignment; local batches are qualified once. This SDK constructor is transport setup, not a host authority API: the host authenticates and checks owner equality on every advertisement. Runtime registration never accepts a caller-supplied owner. Proof obligations: independent instances, retry reuse, partial initialization, cancellation/cleanup, transferred stream retry refusal, dependency absence/duplicates, service reconstruction and no restart after shutdown.

### Slice D: Runtime And Facade — Owner `runtime_facade`

- [x] 1.12 Own `crates/harmos-runtime/` and `crates/harmos/` exclusively. Implement typed invocations, registration/withdrawal, plugin ownership/generations, resident/observer policy and caller migration. Migrate internal built-ins without moving execution into domain. Replace the old runtime Service context name with ServiceContext, exposing C::Service as the lifecycle trait; remove Fault classification/recovery predicates. Gate facade execution behind optional runtime and curate root/prelude exports, including all five required policy names and ServicePolicy. Existing execution features imply runtime.
- [x] 1.13 Provide the following contracts; Runtime/Handle and PluginScope share the invocation surface and authority checks. OperationManager<L> and ServiceManager use register/unregister and remain sole registry owners. RegistrationExt is defined in runtime and implemented for the SDK batch, avoiding orphan inherent methods. Existing public inspection is outside this change: do not add or replace it.

```rust
// Application gains type Lanes: Lane; existing associated types stay.
pub trait Lane: Copy + Eq + Send + Sync + 'static { /* Existing Lane Methods */ }
pub trait RegistrationExt {
    fn lane<L: Lane>(self, lane: L) -> Registration<L>;
    fn defaults<L: Lane>(self) -> Registration<L>;
}
impl<L: Lane> Registration<L> {
    pub fn default_lane_for<O: OperationDeclaration>(self, lane: L) -> Self;
}
impl<A: Application> Runtime<A> {
    pub fn operation<R: RequestFor>(&self, request: R) -> Invocation<R::Operation, A::Lanes>;
    pub fn operation_for<O: OperationDeclaration>(&self, request: O::Request) -> Invocation<O, A::Lanes>;
    pub async fn register(&self, entries: Registration<A::Lanes>) -> Result<(), Error>;
    pub async fn unregister(&self, id: &QualifiedId) -> Result<(), Error>;
}
impl<O: OperationDeclaration, L: Lane, E: EndpointSet> Invocation<O, L, E> {
    pub fn retry(self, policy: RetryPolicy) -> Self;
    pub fn lane(self, lane: L) -> Self;
    pub fn endpoints<F: EndpointSet>(self, endpoints: F) -> Invocation<O, L, F>;
    pub async fn start(self) -> Result<Running<O>, Error>;
    pub async fn run(self) -> Result<Response<O::Response>, Error>;
}
// Invocation's third parameter defaults to (); Running<O> implements migrated Instance<O>.
// Running::join(self) -> Result<Response<O::Response>, Error> is async.
// OperationManager<L>::register(Registration<L>)/unregister(&QualifiedId) are async Result<(), Error>.
// ServiceManager::register(RegistrationBatch)/unregister(&QualifiedId) are async Result<(), Error>.
```

- [x] 1.14 Consume A's policy/identity/declarations, B's catalogs/lanes and C's factories/contexts/Service. Host owner assignment is private, never a caller-supplied register prefix. Service-only batches use the same host lane wrapper but do not consume request lane capacity. Preserve bounded stop/cleanup and code lifetime; unknown tokens fail before admission and entry generations fence stale frames. Proof obligations: inert builders, per-call policy isolation, typed/default lane placement, shared-resource serialization, atomic invalid batch, individual removal, failed cleanup retention, restart credit and dynamic plugin withdrawal.

### Slice E: Carriers And Generated Bindings — Owner `carriers_bindings`

- [ ] 1.15 Own `crates/harmos-sidecar/`, `crates/harmos-guest/` (including WIT) and `crates/harmos-packaging/` exclusively. Migrate SDK lifecycle, kit exports, advertisements, existing transport envelopes and Rust/Python/TypeScript generation. Remove manual operation versions and obsolete macro wrappers. Encode contract fingerprint as 32 bytes, route owner-qualified ID separately, and carry generation as u64; reject version-only peers explicitly. Do not change persisted application record/journal versions.
- [ ] 1.16 Provide carrier re-exports of C::Operation, OperationContext, Dependencies, Service and RegistrationBatch; carrier serving tables expose C's register/unregister signatures and create owner-bound tables/contexts. Generated contract data carries `fingerprint: [u8; 32]` in Rust/binary, lowercase 64-hex text in JSON/Python/TypeScript, and `generation: u64` (lossless bigint/string binding where needed). Consume A::Message/Portable/OperationDefinition/QualifiedId, C::WireTable and D's qualified routing/lane admission contracts; transports never requalify routed IDs. Existing protocol message variants retain their roles, with migrated identity fields.
- [ ] 1.17 Proof obligations: native/sidecar/guest parity, old-peer rejection, wrong fingerprint, stale generation, ownership spoofing, lane token rejection, custom codec refusal, cross-language identity vectors and unchanged streaming cancellation/accounting. No new reload or transport architecture.

### Slice F: Consumers, Scaffolding And Guidance — Owner `consumers_guidance`

- [ ] 1.18 Own `examples/`, `crates/cargo-harmos/`, `crates/harmos-storage/`, `docs/`, `README.md`, `CONTRIBUTING.md`, `.agents/skills/harmos-architecture/`, `.agents/skills/harmos-authoring/`, `.agents/skills/harmos-testing/`, and their same-named `.claude/skills/` copies exclusively. Migrate schematic into `examples/schematic/operations/` with direct concern directories, its own Cargo.toml and runtime feature; migrate declarations/implementations from domain/sidecar/guest, retaining carrier-specific resource differences. Update example workspace manifest/lock and templates in this slice. No extra mirrored operation tree or forwarding operations.
- [ ] 1.19 Provide consumer catalog `Operations::catalog() -> Vec<OperationDefinition>` and runtime `Operations::registration(Dependencies) -> RegistrationBatch`, as generated by B; no new framework public API. Consume A's message/policy/declaration signatures, B's macros, C's lifecycle and D's registration/invocation surface. Migrate storage's message calls without changing durable formats. Follow the two final design examples; retain shared domain models and optional lower-level resource dependencies. Review README impact using stkr-readme and update only affected claims/examples.
- [ ] 1.20 Proof obligations: contract-only dependency isolation, runtime-enabled example execution, unary/streaming carrier consumers, generated starter authoring and unchanged stored-data decoding. Update canonical repository Harmos skills and generated copies together; render only with `stkr project update --no-provider`. External company skill owners are deferred, not silently substituted by installed-cache changes.

### Integration — Owner `integration_verify_fix`

- [ ] 1.21 Own only `Cargo.toml`, `Cargo.lock`, `mise.toml`, `mise.lock`, `stkr.toml`, `uv.lock`, `pyproject.toml`, `tools/`, `.mise/`, `.github/`, `.agents/GUIDANCE.md`, `AGENTS.md`, `CLAUDE.md`, and `openspec/changes/authoring-streamline/` during parallel writing. Make only necessary dependency/feature/task/fixture inventory changes; never edit other open changes. Register any newly discovered file outside these reservations in this inventory before editing. Consume all A-F interfaces; provide a coherent workspace/feature graph, not another public API.
- [ ] 1.22 After every writer hands off and stops writing, ownership of A-F paths transfers exclusively to this one verify-fix agent. Resolve interface mismatches, compile through discovered explicit mise tasks with compiler preflight, run targeted behavior tests and contract-only/runtime feature checks, and repair failures. Audit persisted/wire version consumers before accepting format edits; retain stored payload/journal bytes and explicit old-peer rejection. Do not broaden into full lanes before all implementation items close. Remaining integration includes replacing old runtime Declaration/Resources/Context associations, native batch registration and typed OperationManager, ServiceManager reconstruction, PluginScope invocation/registration, owner assignment and fingerprint/generation fencing, observer residency, and remaining consumers.
- [ ] 1.23 Verify no superseded adapters, declaration versions, Resources/Context associations, duplicate catalogs or stale examples remain. Verify grant/cache request access, policy type identity, complete streams/fingerprints, service cleanup and feature isolation. Review qualitative documentation scenarios without prose assertions. Close slice items only with their targeted proof and handoffs; then declare SOURCE COMPLETE on the combined branch, leaving section 2 open. Deferred reload/protocol redesign/external skill alignment are excluded as recorded in proposal/spec deltas; no claim that they shipped.

## 2. Validation And Delivery

- [ ] 2.1 Orchestrator: obtain delivery authorization after presenting the source-complete candidate.
- [ ] 2.2 Orchestrator: push the combined candidate, verify its remote revision, start the default all-required-platform CI simulation and hosted checks, and create the ready PR into dev with native auto-merge.
- [ ] 2.3 Orchestrator: resolve required proof failures and record actual validation outcomes without treating unavailable executors as passed.
- [ ] 2.4 Orchestrator: finish and archive the specification only after all required tasks are complete.
