/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */
#[path = "../../../../../tests/support/scenarios.rs"]
mod scenario_runtime;

#[path = "../structure.rs"]
mod structure;
#[path = "../../support/mod.rs"]
mod support;

#[allow(dead_code)]
mod probe {
    use harmos::sidecar::inventory::{Declaration, DeclarationError};
    use harmos::sidecar::{
        Attach, Attached, Executable, Frame, Notice, SidecarError, StreamDeclaration, StreamSpec,
        Supervised,
    };
    use harmos::{
        Aggregate, Aggregated, Application, At, Axial, Batch, BeneathWindow, Bucket, Builder,
        Capacity, Catalog, CausationId, CorrelationId, Cursor, DecodeError, Definition,
        DefinitionId, Demand, Draft, Entry, Envelope, Error, Evidence, EvidenceError, EvidenceKind,
        EvidenceRecord, Excerpt, FactSource, Fault, Focus, Handle as Grouping, Horizon, Identified,
        Incompatible, Inspection, Inspector, Invert, Item, JournalConsumer, JournalMetadata, Kind,
        Lane, LaneSet, Link, ManagedState, Never, Origin, Payload, Point, Position, Prefix,
        PrincipalId, Query, Receipt, Recorded, Recovery, Refusal as RuntimeRefusal, Registration,
        RequestKey, Resident as ResidentContract, Reversals, Runtime, Scope, Seal, Series, Service,
        ServiceExit, ServicePolicy, ServiceStatus, Source, StateManager, StopParticipant, Stream,
        StreamConsumer, StreamManager, Survey, SystemFact, Timestamp, Track, Tracked, Transaction,
        Undoable, Vault,
    };

    use crate::support::{Bank, Change, Deposit, Ledger, Refusal};

    // The application-facing names. Five are named through the traits they
    // belong to, which is the only place a trait can be named.
    pub type State = <Bank as Application>::State;
    pub type Refused = <Change as Transaction<Ledger>>::Error;
    pub type Reversal = <Deposit as Undoable>::Inverse;
    pub type Borrowed = <Bank as Application>::Resources;
    pub type Handle = StateManager<Bank>;
    pub type ReadOnlyState = ManagedState<Bank>;
    pub type RequiredLimits = harmos::CaptureLimits;
    pub type StreamLimits = harmos::ReadLimits;
    pub type StreamPageLimits = harmos::PageLimits;
    pub type StreamPage = harmos::RowPage<u64>;
    pub type StreamCut = harmos::ReadCut;
    pub type StreamReadError = harmos::ReadError;
    pub type StreamReader = harmos::RowReader<u64>;
    pub type SharedPublication = harmos::ManagedPublication;
    pub type PendingPublication = harmos::StagedPublication;
    pub fn borrowed_record<R, C: harmos::RecordProjection<R>>(catalog: &C) -> Option<&R> {
        catalog.project()
    }
    pub fn row_source<S: harmos::RowSource>(source: S) -> S {
        source
    }

    pub type CaptureFailure = harmos::CaptureError;
    pub fn required<C: harmos::JournalCapture<Bank>>(consumer: C) -> C {
        consumer
    }
    pub type Answer = Error<Refusal>;
    pub type Narrowed = Focus<Bank>;
    pub type Available = Reversals;
    pub type Stamped = At<i64>;
    pub type Historical = Entry<Change>;
    pub type Anchor = Origin<Ledger>;
    pub type Landed = Position;
    pub type Confirmation = Receipt;
    pub type Witnessed = JournalMetadata;
    pub type Answers = Link;
    pub type Family = CorrelationId;
    pub type Because = CausationId;
    pub type Who = PrincipalId;
    pub type Retry = RequestKey;
    pub type SystemSource = FactSource;
    pub type SystemRecord = SystemFact;
    pub type Ended = RuntimeRefusal;
    pub type Concurrency = Capacity;
    pub type DeclaredLanes = LaneSet;
    pub type AttemptSeam<'a> = Scope<'a>;
    pub type Resident = Service<Bank>;
    pub type ResidentPolicy = ServicePolicy;
    pub type Regime = Fault<Refusal>;
    pub type ResidentExit = ServiceExit;
    pub type ResidentStatus = ServiceStatus;
    pub type NoTransactions = Never;

    // The eight names the supervised child process adds beside them.
    pub type Isolated = Supervised<Bank, Executable, fn(&Service<Bank>, Notice)>;
    pub type Started = Executable;
    pub type Opened = Attached;
    pub type Unsolicited = Notice;
    pub type CallRefusal = SidecarError;
    pub type PayloadDecodeFailure = harmos::MessageError;
    pub fn payload<T: harmos::Message>(value: &T) -> usize {
        value.encoded_len()
    }
    pub type Envelopes = Frame;

    // One portable operation policy and key type across host and plugin facades.
    pub type OperationPolicy = harmos::OperationPolicy;
    pub type GuestOperationPolicy = harmos::guest::OperationPolicy;
    pub type SidecarOperationPolicy = harmos::sidecar::OperationPolicy;
    pub type OperationCacheKey = harmos::CacheKey;

    pub fn operation_policy_identity(
        policy: OperationPolicy,
    ) -> (GuestOperationPolicy, SidecarOperationPolicy) {
        (policy, policy)
    }

    // The six names file-shaped evidence adds beside them.
    pub type Kept = Evidence<Bank>;
    pub type Finalized = EvidenceRecord;
    pub type Unfinished = Draft;
    pub type Window = Excerpt;
    pub type KeptRefusal = EvidenceError;

    pub fn reaches<A: Attach>(attach: A) -> A {
        attach
    }

    pub fn describes<K: EvidenceKind>(details: K::Details) -> K::Details {
        details
    }

    pub fn keeps<V: Vault>(vault: V) -> V {
        vault
    }

    pub fn declares<O: harmos::operations::OperationDeclaration>(operation: O) -> O {
        operation
    }

    pub const HISTORY_REFUSALS: [Error; 4] = [
        Error::OriginAhead,
        Error::BeneathHorizon,
        Error::BeyondTail,
        Error::IncompleteCommit,
    ];

    // The application-facing names StreamManager adds beside the journal core.
    pub type Feeds = StreamManager<Bank>;
    pub type Observed = Item<i64>;
    pub type Asked = Query<i64>;
    pub type Wanted = Demand;
    pub type Beneath = BeneathWindow;
    pub type Partition = <crate::support::Capture as Tracked>::Track;
    pub type Budgeted = Survey<crate::support::Capture>;
    pub type Barrier = Seal<crate::support::Capture>;
    pub type Folded = Bucket;
    pub type Placed = Point;
    pub type Statistic = Aggregate;
    pub type Answered = Aggregated;

    pub fn partitions<S: Tracked>(row: S) -> S {
        row
    }

    pub fn pushes<R: StreamSpec<Row = R>>(row: &R) -> (&'static str, u32, &R) {
        (R::ID, R::VERSION, row)
    }

    pub fn sidecar_declaration(value: Declaration) -> Declaration {
        value
    }

    pub fn sidecar_declaration_error(value: DeclarationError) -> DeclarationError {
        value
    }

    pub fn operation_declaration(
        value: harmos::operations::Definition,
    ) -> harmos::operations::Definition {
        value
    }

    pub fn sidecar_stream_declaration(value: StreamDeclaration) -> StreamDeclaration {
        value
    }

    pub fn declared_sidecar<S: harmos_contract::plugin::Declared>()
    -> (&'static str, &'static str, &'static str) {
        (S::ID, S::VERSION, S::DESCRIPTION)
    }

    pub fn places<S: Axial>(row: S) -> u64 {
        S::AXIS.len() as u64 + row.axis()
    }

    pub fn labels<T: Track>(track: T) -> String {
        track.label()
    }

    pub fn narrows<S: Tracked, K: Prefix<S::Track>>(selection: K) -> Vec<String> {
        selection.labels()
    }

    pub fn serves<C: Series<Bank>>(series: C) -> C {
        series
    }

    // The adapter-only names: the two fold contracts and their coordinates,
    // then the encode boundary and recovery equation.
    pub type Reached = Cursor;
    pub type Rows = Batch<Bank>;
    pub type Crossing = Envelope<Vec<u8>>;
    pub type Which = Kind;
    pub type Named = DefinitionId;
    pub type Carried = Recorded<Bank>;
    pub type Restored = Payload<Bank>;
    pub type Equation = Recovery<Bank>;
    pub type Undecodable = DecodeError;

    // The eleven composition types; `Registration` is named through `catalogs`.
    pub type Whole = Runtime<Bank>;
    pub type ReadOnly = Inspector<Bank>;
    pub type ReadSurface = Inspection<Bank>;
    pub type Assembly = Builder<Bank>;
    pub type Grouped = Grouping<Bank>;
    pub type Retention = Horizon;
    pub type Declared = Definition;
    pub type Registered = Catalog<Bank>;
    pub type Rejection = Incompatible;
    pub type StopToken = StopParticipant;

    // The clock, re-exported rather than newtyped.
    pub type When = Timestamp;

    pub fn folds<C: JournalConsumer<Bank>>(consumer: C) -> C {
        consumer
    }

    pub fn captures<C: StreamConsumer<Bank>>(consumer: C) -> C {
        consumer
    }

    pub fn recovers<S: Source<Bank>>(source: S) -> S {
        source
    }

    pub fn publishes<S: Stream<Bank>>(row: S) -> S {
        row
    }

    pub fn catalogs<C: Registration<Bank>>(catalog: C) -> C {
        catalog
    }

    pub fn residents<R: ResidentContract<Bank>>(resident: R) -> R {
        resident
    }

    pub fn routes_on<L: Lane>(lane: L) -> (&'static str, L) {
        (lane.name(), lane)
    }

    pub fn identifies<D: Identified>() -> (DefinitionId, u32) {
        D::definition()
    }

    pub fn inverts<C: Invert<Ledger>>(change: &C, before: &Ledger) -> C::Inverse {
        change.invert(before)
    }
}

#[allow(dead_code)]
mod guest_probe {
    use harmos::guest::inventory::{Declaration, DeclarationError};
    use harmos::guest::{Error, Host, Loaded, OperationRefusal, Registered, Registration, Serving};

    use crate::support::Bank;

    // The three states one loaded artifact passes through, in order. Naming
    // them as a progression is the point: each is reachable only from the one
    // before it, and a stage that stopped consuming its predecessor would stop
    // being a typestate without any other test saying so.
    pub type Inspected = Loaded<Bank>;
    pub type Catalogued = Registered<Bank>;
    pub type Answering = Serving<Bank>;
    pub type RoutedGuest = Serving<Bank>;

    // The loader and the failures it reports.
    pub type Loader = Host<Bank>;
    pub type GuestRefusal = Error;

    // The static declaration a loader reads before it compiles anything.
    pub type GuestDeclaration = Declaration;
    pub type GuestDeclarationError = DeclarationError;
    pub type GuestRegistration = Registration;

    // The route surface the guest facility gained, named through the traits it
    // carries and the table a guest serves it from.
    pub type ServedOperations = harmos::guest::operations::WireTable<harmos::guest::Context>;
    pub type RouteEnvelope = OperationRefusal;

    pub fn implements<G: harmos::Plugin>(config: G::Config) -> G::Config {
        config
    }

    pub fn declared_guest<D: harmos_contract::plugin::Declared>() {}

    pub fn calls<O: harmos::guest::operations::OperationDeclaration>(operation: O) -> O {
        operation
    }
}

#[cfg(feature = "sim")]
#[allow(dead_code)]
mod sim_probe {
    use harmos::SimulatedRuntime;
    use harmos::sim::{ServiceProbe, SimError, entropy};

    use crate::support::Bank;

    pub type Harness = SimulatedRuntime<Bank>;
    pub type FaultDelivery = SimError;
    pub type ResidentTrace = ServiceProbe;

    pub fn draw() -> u64 {
        entropy()
    }
}

/* === Re-Exports === */

/* === Imports === */
// Standard Library Imports
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
// Third Party Imports

// Local Imports

/* === Constants === */
const CENSUS: [&str; 143] = [
    "Plugin",
    "listeners",
    "btree_map_memory",
    "BootstrapCleanup",
    "BootstrapStatus",
    "MemorySize",
    "Startup",
    "Storable",
    "StreamRegistration",
    "StagedPublication",
    "OperationManager",
    "CacheKey",
    "OperationDeclaration",
    "OperationPolicy",
    "RetryPolicy",
    "BackendFailure",
    "CaptureError",
    "CaptureLimits",
    "JournalCapture",
    "Batched",
    "Batches",
    "BatchItem",
    "Summary",
    "Measurement",
    "Statistics",
    "Message",
    "MessageError",
    "StreamSpec",
    "Value",
    "ValueError",
    "Metadata",
    "Identifier",
    "IdentifierError",
    "StreamField",
    "RowMetadata",
    "schema",
    // Application-Facing StateManager And Stream Contracts.
    "At",
    "CausationId",
    "CorrelationId",
    "Entry",
    "Error",
    "Identified",
    "CatalogCodec",
    "TransactionCatalog",
    "RecordCatalog",
    "RecordProjection",
    "Record",
    "Records",
    "Prepared",
    "PreparationError",
    "Invert",
    "Undoable",
    "StateManager",
    "ManagedState",
    "Link",
    "JournalMetadata",
    "Origin",
    "Position",
    "PrincipalId",
    "Application",
    "Receipt",
    "RequestKey",
    "Reversals",
    "Focus",
    "FactSource",
    "SystemFact",
    "Transaction",
    "Never",
    "BeneathWindow",
    "Demand",
    "Item",
    "Query",
    "PageLimits",
    "RowPage",
    "ReadCut",
    "ReadError",
    "ReadLimits",
    "RowReader",
    "RowSource",
    "Stream",
    "RecordManager",
    "StreamManager",
    "StreamFeed",
    "PluginStream",
    "PublicationLease",
    "PublicationRow",
    "ManagedPublication",
    "StreamAdmissionError",
    "StreamPublicationError",
    "Track",
    "Prefix",
    "Tracked",
    "Axial",
    "Survey",
    "Seal",
    "Bucket",
    "Point",
    "Aggregate",
    "Aggregated",
    // Public lane admission and resident lifecycle contracts.
    "Capacity",
    "Fault",
    "Lane",
    "LaneSet",
    "Refusal",
    "Resident",
    "Scope",
    "Service",
    "ServiceExit",
    "ServicePolicy",
    "ServiceStatus",
    // Six names for file-shaped evidence: the facade, one piece of evidence at
    // whichever stage it has reached, the kind declaration naming the typed
    // account a piece of evidence of that kind records, a draft still being written,
    // the one bounded window both stages read through, and the refusal.
    "EvidenceRecord",
    "EvidenceError",
    "Evidence",
    "Draft",
    "EvidenceKind",
    "Excerpt",
    // Twelve composition names.
    "Builder",
    "Registration",
    "Catalog",
    "Definition",
    "Horizon",
    "Incompatible",
    "Inspection",
    "Inspector",
    "Handle",
    "Runtime",
    "StopParticipant",
    // Fourteen adapter-only names: fold contracts and coordinates, the encode
    // boundary and recovery equation, and the evidence vault port — which
    // lives here beside the other two rather than in the crate that
    // implements it.
    "Batch",
    "Cursor",
    "DecodeError",
    "DefinitionId",
    "Envelope",
    "JournalConsumer",
    "Kind",
    "Payload",
    "Recorded",
    "Recovery",
    "Series",
    "Source",
    "StreamConsumer",
    "Vault",
    // The standard-library clock.
    "Timestamp",
    "Protobuf",
];

const HIDDEN: [&str; 28] = [
    "prost",
    // How generated code names a type the tokens it was handed cannot spell,
    // and how a hand-written contribution names the same one.
    "Answer",
    "Catalogued",
    "Mutual",
    "StreamCatalog",
    "StreamRow",
    "BatchDefinition",
    "CarrierContext",
    "PluginContribution",
    "PluginDeclared",
    "PluginGrant",
    "PluginInventory",
    "PluginRegistrar",
    "PluginRegistrationError",
    "PluginRequirement",
    "PluginRequirements",
    // Native Decoding Preserves Fragment Merging And Charges Memory Before Allocation.
    "DecodeField",
    "DecodeInput",
    "DecodeScalar",
    "DecodeValue",
    "decoding_add",
    "decoding_multiply",
    "decoding_vector",
    "MessageError",
    "Protobuf",
    "PostcardReader",
    "PostcardValue",
    "with_protobuf",
];

const AUTHORING: [&str; 13] = [
    "catalog",
    "message",
    "plugin",
    "lanes",
    "listener",
    "record",
    "records",
    "observer",
    "stream",
    "streams",
    "transaction",
    "transactions",
    "operation",
];

const GUEST_CENSUS: [&str; 35] = [
    "CacheKey",
    "ConfigError",
    "Context",
    "Error",
    "MemorySize",
    "Message",
    "MessageError",
    "OperationDeclaration",
    "OperationPolicy",
    "OperationRefusal",
    "Plugin",
    "Registrar",
    "Registration",
    "RegistrationError",
    "Result",
    "RetryPolicy",
    "RowMetadata",
    "Storable",
    "StreamSpec",
    "UNKNOWN",
    "UNREADABLE",
    "WIT",
    "__private",
    "catalog",
    "componentize",
    "encode_config",
    "inventory",
    "message",
    "operations",
    "reserved",
    "schema",
    "validate_config",
    "operation",
    "PublishError",
    "Publisher",
];

const SIDECAR_KIT_CENSUS: [&str; 66] = [
    "Activation",
    "ActivationId",
    "Attach",
    "Attached",
    "CATALOG_EXPORT_FLAG",
    "CATALOG_EXPORT_LIMIT",
    "CacheKey",
    "CallError",
    "ConfigError",
    "DemandKey",
    "Error",
    "Executable",
    "ExecutableError",
    "Frame",
    "Instance",
    "MemorySize",
    "Message",
    "MessageError",
    "Observer",
    "ObserverAttachment",
    "ObserverCaller",
    "ObserverContext",
    "ObserverControl",
    "ObserverExit",
    "ObserverHost",
    "ObserverPublisher",
    "ObserverStatus",
    "ObserverStreamPublisher",
    "ObserverTask",
    "ObserverUpdate",
    "OperationDeclaration",
    "OperationPolicy",
    "Plugin",
    "PluginCatalog",
    "PluginNative",
    "Publication",
    "PublicationError",
    "RESERVED",
    "Refusal",
    "Registrar",
    "Registration",
    "RegistrationError",
    "Result",
    "RetryPolicy",
    "RowMetadata",
    "Storable",
    "StreamDeclaration",
    "StreamSpec",
    "Supervisor",
    "Tick",
    "UNKNOWN",
    "UNREADABLE",
    "__private",
    "catalog",
    "encode_config",
    "inventory",
    "message",
    "operations",
    "reserved",
    "run_plugin",
    "schema",
    "serve_plugin_channels",
    "validate_config",
    "operation",
    "PublishError",
    "Publisher",
];

const DELETED: [&str; 48] = [
    "Journal",
    "RouteError",
    "RouteRefusal",
    "routes",
    "route",
    "DEADLINE",
    "route_markers",
    "registered",
    "Routes",
    "RouteRegistration",
    "ROUTES",
    "Extract",
    "Call",
    "Carrier",
    "RouteDeclaration",
    "Route",
    "RouteTable",
    "AbsorbsRefusal",
    "Errand",
    "IntoJob",
    "Job",
    "JobHandle",
    "JobPolicy",
    "Work",
    "ActorId",
    "Commit",
    "CommitId",
    // The grant-at-load permission set an operator handed a guest. Permissions
    // are gone entirely — a declared edge is admitted by being declared — and
    // the firewall that may replace them gets designed fresh at the single
    // dispatch slot rather than reviving this type.
    "Consents",
    "Consumer",
    "EntryId",
    "EntryKind",
    "Frontier",
    // The one trait that carried both a change's inverse declaration and its
    // body. A change declares in its own attribute now and describes through
    // `Invert`, so neither the trait nor the builder verb it fed comes back.
    "Invertible",
    "Inverses",
    // The combined job answer, generic over the application's error. A job's
    // own error and the runtime's refusals are two facts, and the seam hands
    // each back as itself now, so neither the type nor the hand-written bridge
    // every application owed it comes back.
    "JobError",
    // Function-shaped jobs and their result projections are retired.
    "JobResult",
    "Output",
    "Page",
    "PluginId",
    "PluginRow",
    "Reversible",
    // Legacy contribution catalogs are retired; explicit operation catalogs
    // and current observer declarations own the supported authoring surface.
    "RouteCatalog",
    "SamplerCatalog",
    "Sampler",
    "SamplerContext",
    "ListenerCatalog",
    // The second attempt contract, elected by a `scoped` flag. Every attempt
    // receives its original operation context, so no second contract remains.
    "ScopedErrand",
    "Sealed",
];

const UNSPEAKABLE: [&str; 13] = [
    "Actor",
    "Channel",
    "Daemon",
    "Milestone",
    "Registry",
    "ResourceStore",
    "Schedule",
    "StreamKey",
    "Subscription",
    "Tag",
    "Watchdog",
    "Worker",
    "Writer",
];

const DOWNSTREAM: [&str; 44] = [
    "Aggregate",
    "Aggregated",
    "EvidenceRecord",
    "EvidenceError",
    "Evidence",
    "Axial",
    "Batch",
    "BeneathWindow",
    "Bucket",
    "Builder",
    "Cursor",
    "Definition",
    "Demand",
    "Draft",
    "EvidenceKind",
    "Excerpt",
    "Incompatible",
    "JournalConsumer",
    "Point",
    "Prefix",
    "Query",
    "PageLimits",
    "RowPage",
    "ReadCut",
    "ReadError",
    "ReadLimits",
    "RowReader",
    "RowSource",
    "Handle",
    "Runtime",
    "Seal",
    "Series",
    "StreamConsumer",
    "RecordManager",
    "StreamManager",
    "StreamFeed",
    "PluginStream",
    "PublicationLease",
    "StreamAdmissionError",
    "StreamPublicationError",
    "Survey",
    "Track",
    "Tracked",
    "Vault",
];

const ALLOWANCE: [(&str, &[&str]); 17] = [
    (
        "harmos",
        // `harmos-sidecar` owns the process boundary and this crate owns its
        // runtime supervision. Harmos owns payload operations through the shared
        // artifact contract; activation settings keep their JSON codec. The dependency
        // points from runtime to boundary; the author kit does not depend back
        // on the runtime.
        //
        // `jsonl-adapter` is the reference adapter under `examples/`, and it is
        // here for the durability pillar alone: a contract about stored bytes
        // needs an adapter to store them. The direction still runs one way —
        // the adapter depends on this crate — and a lockfile merges dev
        // dependencies into one list, which is why a test-only name appears in
        // this allowance beside `tempfile` and `harmos-guest`.
        &[
            // Borrowed gRPC Decode Buffers; Optional With grpc Only.
            "bytes",
            // Optional gRPC Identifier Collision Checks Match Prost Casing.
            "heck",
            // Criterion is confined to explicit benchmark targets.
            "criterion",
            "futures-core",
            // Concurrent operation adapters and instance/context entropy.
            "futures-util",
            "getrandom",
            "harmos-contract",
            "harmos-guest",
            "harmos-macros",
            "harmos-sidecar",
            "harmos-signal",
            // Optional gRPC Response Body Owns The Active RPC Permit.
            "http-body",
            // In-Memory Tonic Framing Regression Bodies Only.
            "http-body-util",
            "jsonl-adapter",
            "petgraph",
            "semver",
            // Optional Standard gRPC Service Descriptor Composition.
            "prost-reflect",
            // Test-Only protoc Compile Of Exported Storage Service Sources.
            "protoc-bin-vendored",
            "serde",
            "serde_json",
            "sha2",
            "tar",
            "tempfile",
            "thiserror",
            "tokio",
            // Optional External gRPC Adapter; Disabled Without The grpc Feature.
            "tonic",
            "tonic-reflection",
            // Optional gRPC-Web Translation On The Same Listener.
            "tonic-web",
            // Test-Only Rust Syntax Census.
            "syn",
            "wasmtime",
            "tracing",
            // Test-only capture verifies observer diagnostic scope propagation.
            "tracing-subscriber",
        ],
    ),
    (
        // The declaration surface both facilities name. Postcard is the default;
        // protobuf is an explicit projection. The macro dependency supplies
        // the guest-safe authoring facade; nothing here reaches a transport.
        "harmos-contract",
        // Shared native/guest Requirement owns its semantic version constraint.
        &[
            "harmos-macros",
            "memchr",
            // Bounded Internal Serving Envelopes.
            "postcard",
            "prost",
            "prost-reflect",
            "semver",
            "serde",
            "serde_json",
            "thiserror",
            "time",
            "tracing",
            // Deterministic Proto Names And Explicit Optional Stream Channels.
            "heck",
            "tokio",
            // Textual Proto Export Verification Uses The Standard Compiler.
            "protoc-bin-vendored",
            "tempfile",
        ],
    ),
    (
        "harmos-sidecar",
        &[
            // Native SDK Tests Author The Same Unified Plugin Contract.
            "harmos-macros",
            "criterion",
            "tempfile",
            "futures-util",
            // Native activation identities must not repeat after process restart.
            "getrandom",
            "harmos-contract",
            "hyper-util",
            // Native Descriptor Attachment And Binary Carrier Envelopes.
            "libc",
            "postcard",
            "prost",
            "protoc-bin-vendored",
            "semver",
            "serde",
            "serde_json",
            "thiserror",
            "tokio",
            "tokio-stream",
            "tonic",
            "tonic-prost",
            "tonic-prost-build",
            "tower",
            // Windows process startup restores the OS environment baseline.
            "windows-sys",
            "tracing",
        ],
    ),
    (
        // The contract crate: the editor's contract with its own plugins. Harmos
        // with every default off, serde, and shared error/tracing contracts. A
        // contract that reached an adapter or a runtime could not also be
        // compiled by the guest that has to honor it. `mise run check:wasm` is
        // where that stops being an intention.
        "minischematic-contract",
        &["harmos", "serde", "thiserror", "tracing"],
    ),
    (
        // The application crate: state, catalogs, entry verbs, wire format,
        // storage composition, the document, the host half of the guest edge,
        // and the native readback of a finished recording. `tracing` instruments
        // the entry verbs and nothing else — installing a subscriber over them
        // is the shell's decision, which is why `tracing-subscriber` is absent
        // here and present in `minischematic-cli`.
        "minischematic-app",
        &[
            "futures-core",
            "harmos",
            "jsonl-adapter",
            "minischematic-contract",
            "serde",
            "serde_json",
            // Inspected Native Worker Artifact Identity.
            "sha2",
            "thiserror",
            "tokio",
            "tracing",
            // Explicit Host-To-Guest Communication Benchmark.
            "criterion",
        ],
    ),
    (
        "harmos-signal",
        // Signal kernels and shared error/instrumentation contracts, with no runtime.
        &["rustfft", "thiserror", "tracing"],
    ),
    (
        "harmos-macros",
        &[
            // Authored Operation Names Derive The Same Snake Case As Transport Names.
            "heck",
            "proc-macro-crate",
            "proc-macro2",
            "quote",
            "semver",
            "serde_json",
            "syn",
        ],
    ),
    (
        // The reference adapter, which is a worked example and not a library
        // the workspace depends on: nothing here depends back on it except the
        // tests that need something to store bytes with. One format, one codec,
        // no features — `serde` and `serde_json` are the codec itself rather
        // than an optional backend, and `tokio` is what its own tests are
        // driven by. A second format would be a second adapter, not an entry
        // here.
        "jsonl-adapter",
        &[
            "harmos",
            "tempfile",
            "serde",
            "serde_json",
            "tokio",
            "thiserror",
            "tracing",
        ],
    ),
    (
        // `wasmtime` is the guest tests' own host and enters the graph only for
        // them — the world is proven by running a component built from this
        // kit, which takes a component runtime. `wit-component` is the encoder
        // that wraps a core module into that component, and it is here rather
        // than in a task file so it resolves beside the wasmtime that has to
        // accept what it writes. The kit a guest author compiles for wasm32
        // links `wit-bindgen` and the shared artifact contract; the target gate on the
        // others is what keeps true.
        //
        // `prost` is the payload language at every tenant boundary, so it
        // reaches wasm32 on purpose: a guest decodes a route's request and
        // encodes its reply inside the instance. Its only required dependency
        // is `bytes` and it supports `no_std`, which is what lets the guest
        // targets keep building after routes exactly as they did before them —
        // `mise run check:wasm` is where that stops being an intention.
        "harmos-guest",
        &[
            "harmos-contract",
            "semver",
            "serde",
            "serde_json",
            "thiserror",
            "tokio",
            "wasmtime",
            "wit-bindgen",
            "wit-component",
            "tracing",
        ],
    ),
    (
        // The shell: it chooses the document, resolves the configuration,
        // installs the subscriber, and renders an error once. It names the app
        // crate and nothing beneath it — the contract crate is the plugins' contract,
        // not a shell's.
        "minischematic-cli",
        &[
            // Shell Argument Parsing.
            "clap",
            "futures-core",
            "harmos",
            "minischematic-app",
            "serde_json",
            "tempfile",
            "thiserror",
            "tokio",
            "tracing-subscriber",
            "tracing",
        ],
    ),
    (
        // The guest: it compiles the editor's contract crate and never learns there
        // is a runtime, which is what lets it compile for wasm32.
        "minischematic-plugin",
        &[
            "harmos",
            "minischematic-contract",
            "serde",
            "serde_json",
            "thiserror",
            "tracing",
        ],
    ),
    (
        // Native Worker Uses Only The Portable Contract And Authoring SDK.
        "minischematic-worker",
        &[
            "harmos",
            "minischematic-contract",
            "thiserror",
            "tokio",
            "tracing",
        ],
    ),
    (
        // The bench's contract crate: the probe edge's settings and route payloads,
        // and nothing else. `Reading` is deliberately absent — a publication
        // enters a stream, and a stream row carries the host's retention policy,
        // so each half declares its own and the two agree on id and schema.
        "minibench-contract",
        &["harmos", "serde", "thiserror", "tracing"],
    ),
    (
        // The bench's host half. `tokio` is the one wait a job makes while its
        // declared child finishes opening; the deadline bounding it is the job
        // policy's.
        "minibench-app",
        &[
            "harmos",
            "minibench-contract",
            "serde",
            // The Native Artifact Identity Is Its Inspected Content Digest.
            "sha2",
            "thiserror",
            "tokio",
            "tracing",
        ],
    ),
    (
        // The shell no longer links the sidecar crate: its calls are placed by
        // jobs the application admits, and the executable it points at is a
        // path the build produced rather than a crate this shell compiles.
        "minibench-cli",
        &[
            // Shell Argument Parsing.
            "clap",
            "harmos",
            "minibench-app",
            // Machine-Readable Readiness For Signal Acceptance.
            "serde_json",
            "thiserror",
            "tokio",
            "tracing-subscriber",
            "tracing",
        ],
    ),
    (
        // The proof that a sidecar author compiles the kit and the declaration
        // types and nothing else: this member turns harmos's default features
        // off, declares its own stream row and its own sidecar, and would stop
        // compiling the day a declaration attribute reached into the host core.
        // `harmos` is here once, with `runtime` off; error and tracing
        // dependencies do not enable the host runtime.
        "standalone-sidecar",
        &["harmos", "serde", "tokio", "thiserror", "tracing"],
    ),
    (
        "minibench-probe-sidecar",
        &[
            "harmos",
            "minibench-contract",
            "serde",
            "tokio",
            "thiserror",
            "tracing",
        ],
    ),
];

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */
struct Declared {
    public: bool,
    keyword: String,
    name: String,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

/* === Private Functions === */
fn source() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("src")
}

fn workspace() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("the crate lives two levels under the workspace root")
        .to_path_buf()
}

fn sources() -> Vec<(String, String)> {
    let mut found = Vec::new();
    collect(&workspace().join("crates"), &mut found);
    collect(&workspace().join("examples"), &mut found);
    found.sort();
    found
}

fn collect(directory: &Path, found: &mut Vec<(String, String)>) {
    let listing = fs::read_dir(directory).expect("the crates directory is readable");

    for entry in listing.flatten() {
        let path = entry.path();
        if path.is_dir() {
            if path.file_name().is_some_and(|name| name != "tests") {
                collect(&path, found);
            }
        } else if path.extension().is_some_and(|suffix| suffix == "rs") {
            found.push((path.to_string_lossy().replace('\\', "/"), read(&path)));
        }
    }
}

fn read(path: &Path) -> String {
    fs::read_to_string(path)
        .unwrap_or_else(|error| panic!("{}: {error}", path.display()))
        .replace("\r\n", "\n")
}

fn code(text: &str) -> String {
    text.lines()
        .map(|line| line.split("//").next().unwrap_or_default())
        .collect::<Vec<_>>()
        .join("\n")
}

fn identifiers(code: &str) -> Vec<String> {
    code.split(|character: char| !character.is_alphanumeric() && character != '_')
        .filter(|token| !token.is_empty())
        .map(str::to_owned)
        .collect()
}

fn declarations() -> Vec<(String, Declared)> {
    let mut found = Vec::new();

    for (path, text) in sources() {
        let code = code(&text);
        let mut depth: usize = 0;
        let mut impls: Vec<usize> = Vec::new();

        for line in code.lines() {
            let trimmed = line.trim_start();
            if let Some(declared) = declaration(trimmed)
                && (declared.keyword != "type" || impls.is_empty())
            {
                found.push((path.clone(), declared));
            }
            if trimmed.starts_with("impl ") || trimmed.starts_with("impl<") {
                impls.push(depth);
            }
            for character in line.chars() {
                match character {
                    '{' => depth += 1,
                    '}' => {
                        depth = depth.saturating_sub(1);
                        if impls.last().is_some_and(|opened| depth <= *opened) {
                            impls.pop();
                        }
                    }
                    _ => {}
                }
            }
        }
    }
    found
}

fn declaration(line: &str) -> Option<Declared> {
    let (public, rest) = match line.strip_prefix("pub") {
        Some(rest) if rest.starts_with(' ') => (true, rest.trim_start()),
        Some(rest) if rest.starts_with('(') => (false, rest.split_once(')')?.1.trim_start()),
        Some(_) | None => (false, line),
    };
    let rest = rest.strip_prefix("async ").unwrap_or(rest);
    let (keyword, rest) = rest.split_once(' ')?;
    if !["struct", "enum", "trait", "type", "fn", "mod", "const"].contains(&keyword) {
        return None;
    }
    let name: String = rest
        .chars()
        .take_while(|character| character.is_alphanumeric() || *character == '_')
        .collect();

    (!name.is_empty()).then_some(Declared {
        public,
        keyword: keyword.to_owned(),
        name,
    })
}

fn halves(text: &str) -> (String, String) {
    let Some(opened) = text.find("pub mod __private {") else {
        return (text.to_owned(), String::new());
    };
    let body = opened + "pub mod __private {".len();
    let mut depth = 1usize;
    let closed = text[body..]
        .char_indices()
        .find_map(|(at, character)| {
            match character {
                '{' => depth += 1,
                '}' => depth -= 1,
                _ => {}
            }

            (depth == 0).then_some(body + at)
        })
        .expect("the hidden authoring module closes its own block");

    (
        format!("{}{}", &text[..opened], &text[closed..]),
        text[body..closed].to_owned(),
    )
}

fn reexported(text: &str) -> BTreeSet<String> {
    let mut handed_out = BTreeSet::new();
    let mut remaining = text;
    let mut depth = 0usize;

    while !remaining.is_empty() {
        if depth == 0 && remaining.starts_with("pub use ") {
            let Some((statement, after)) = remaining["pub use ".len()..].split_once(';') else {
                break;
            };
            let leaves = match statement.split_once('{') {
                Some((_, group)) => group.to_owned(),
                None => statement.rsplit("::").next().unwrap_or_default().to_owned(),
            };
            handed_out.extend(
                leaves
                    .split(',')
                    .map(|name| name.trim().trim_end_matches('}').trim())
                    // What a census counts is the name a consumer writes, so a
                    // re-export under an alias is counted as the alias.
                    .map(|name| name.rsplit(" as ").next().unwrap_or(name).trim().to_owned())
                    .filter(|name| !name.is_empty()),
            );
            remaining = after;
            continue;
        }

        let character = remaining.chars().next().expect("the text is not empty");
        match character {
            '{' => depth += 1,
            '}' => depth = depth.saturating_sub(1),
            _ => {}
        }
        remaining = &remaining[character.len_utf8()..];
    }

    handed_out
}

fn locked_dependencies(name: &str) -> Option<BTreeSet<String>> {
    let lock = read(&workspace().join("Cargo.lock"));
    let block = lock
        .split("[[package]]")
        .find(|block| block.contains(&format!("name = \"{name}\"")))?;
    let (_, listed) = block.split_once("dependencies = [")?;

    Some(
        listed
            .split(']')
            .next()
            .unwrap_or_default()
            .lines()
            .map(|line| {
                // A lockfile qualifies a dependency with its version when two
                // versions of one package coexist in the graph; the allowance
                // is by name, so the qualifier is not part of the answer.
                line.trim()
                    .trim_end_matches(',')
                    .trim_matches('"')
                    .split_whitespace()
                    .next()
                    .unwrap_or_default()
                    .to_owned()
            })
            .filter(|dependency| !dependency.is_empty())
            .collect(),
    )
}

// These gates select optional plugin authority, native transport or a future's
// Send bound. Record capture, storage and recovery retain their common path.
// Match complete attributes and exact source owners so a new durability branch
// cannot hide behind a multiline cfg or a facility's directory.
fn operation_facility_gate(path: &str, attribute: &str) -> bool {
    let mut key: String = attribute.chars().filter(|c| !c.is_whitespace()).collect();
    while key.contains(",)") {
        key = key.replace(",)", ")");
    }
    let owners: &[&str] = match key.as_str() {
        // Runtime Admission Verification Is Shared By Memory And Durable Storage.
        "#[cfg(feature=\"runtime\")]" => &[
            "error.rs",
            "grpc.rs",
            "grpc/envelope.rs",
            "storage/catalog.rs",
            // Portable Contracts Compile Without Their Runtime Adapters. The
            // Selected Adapter Is Identical For Memory And Persistent Backends.
            "storage/data.rs",
            "storage/object_values.rs",
            "storage/timestamp.rs",
            "storage/operations/abandon_file.rs",
            "storage/operations/acknowledge.rs",
            "storage/operations/append_file.rs",
            "storage/operations/create_file.rs",
            "storage/operations/data_selection.rs",
            "storage/operations/delete_file.rs",
            "storage/operations/file_selection.rs",
            "storage/operations/finish_file.rs",
            "storage/operations/follow.rs",
            "storage/operations/follow_file.rs",
            "storage/operations/follow_objects.rs",
            "storage/operations/follow_stream.rs",
            "storage/operations/mod.rs",
            "storage/operations/object_result.rs",
            "storage/operations/object_selection.rs",
            "storage/operations/query_files.rs",
            "storage/operations/query_schemas.rs",
            "storage/operations/query_storage.rs",
            "storage/operations/query_stream.rs",
            "storage/operations/read_files.rs",
            "storage/operations/read_objects.rs",
            "storage/operations/read_records.rs",
            "storage/operations/reductions.rs",
            "storage/operations/resume_file.rs",
            "storage/operations/save.rs",
            "storage/operations/selection.rs",
            "storage/operations/write_objects.rs",
            "storage/operations/write_records.rs",
            "storage/operations/write_schema.rs",
            "storage/operations/write_stream.rs",
        ],
        "#[cfg(any(feature=\"runtime\",test))]" => {
            &["storage/catalog.rs", "storage/object_values.rs"]
        }
        // Test Fixtures Compile Only With Their Facility's Callers.
        "#[cfg(all(test,any(feature=\"load\",feature=\"guest-host\",feature=\"sidecar-host\")))]" => {
            &["streams/definition.rs", "streams/publication.rs"]
        }
        "#[cfg(all(test,feature=\"sidecar-host\"))]" => &["streams/window.rs"],
        "#[cfg(any(feature=\"sidecar-host\",test))]" => &["supervision/instance.rs"],
        // Native And Remote Observer Adapters Share Retained Cleanup Ownership.
        "#[cfg(any(feature=\"sidecar\",feature=\"sidecar-host\"))]" => {
            &["observers/manager.rs", "observers/mod.rs"]
        }
        "#[cfg(all(feature=\"runtime\",any(feature=\"load\",feature=\"guest-host\",feature=\"sidecar-host\")))]" => {
            &["error.rs", "lib.rs"]
        }
        "#[cfg(any(feature=\"load\",feature=\"guest-host\",feature=\"sidecar-host\"))]"
        | "#[cfg(not(any(feature=\"load\",feature=\"guest-host\",feature=\"sidecar-host\")))]" => {
            &[
                "grpc/server.rs",
                "state/manager.rs",
                "observers/manager.rs",
                "observers/manager/registration.rs",
                "operations/caller.rs",
                "operations/handler.rs",
                "operations/manager.rs",
                "operations/middleware.rs",
                "operations/mod.rs",
                "operations/registration.rs",
                "operations/scope.rs",
                "operations/wire.rs",
                "records/mod.rs",
                "records/registration.rs",
                "runtime/lifecycle.rs",
                // A Replica Receipt Binds To Its Calling Plugin Generation.
                "storage/operations/acknowledge.rs",
                "streams/definition.rs",
                "streams/mod.rs",
                "streams/publications.rs",
                "streams/window.rs",
                "supervision/mod.rs",
            ]
        }
        "#[cfg(all(feature=\"guest-host\",not(target_arch=\"wasm32\")))]" => {
            &["load/router.rs", "operations/mod.rs", "operations/sdk.rs"]
        }
        "#[cfg(any(feature=\"sidecar-host\",all(feature=\"guest-host\",not(target_arch=\"wasm32\"))))]" => {
            &[
                "load/scope.rs",
                "operations/mod.rs",
                "operations/registration.rs",
                "operations/wire.rs",
            ]
        }
        // Generation Callers And Lifecycle Loans Select Facilities, Never Persistence.
        "#[cfg(any(feature=\"sidecar-host\",feature=\"guest-host\"))]" => {
            &["operations/children.rs", "operations/wire.rs"]
        }
        "#[cfg(not(feature=\"sidecar-host\"))]" => {
            &["operations/manager.rs", "operations/caller.rs"]
        }
        "#[cfg(any(feature=\"guest\",feature=\"guest-host\"))]" => &["guest.rs"],
        "#[cfg(any(feature=\"componentize\",feature=\"guest\",feature=\"guest-host\"))]" => {
            &["lib.rs"]
        }
        "#[cfg(feature=\"sidecar\")]" => &["observers/mod.rs"],
        "#[cfg(feature=\"sidecar-host\")]" => &[
            "load/scope.rs",
            "operations/authored.rs",
            "operations/caller.rs",
            "operations/handler.rs",
            "services/supervisor.rs",
            "streams/publications.rs",
            "observers/manager.rs",
            "operations/sdk.rs",
            "operations/mod.rs",
            "operations/children.rs",
            "operations/manager.rs",
            "operations/registration.rs",
            "operations/wire.rs",
        ],
        "#[cfg(feature=\"sim\")]" | "#[cfg(not(feature=\"sim\"))]" => &["runtime/boot.rs"],
        "#[cfg(target_arch=\"wasm32\")]" => {
            return path.ends_with("/crates/harmos-guest/src/kit/operations.rs")
                || path.ends_with("/crates/harmos-guest/src/kit/context.rs");
        }
        _ => return false,
    };
    owners
        .iter()
        .any(|owner| path.ends_with(&format!("/crates/harmos/src/{owner}")))
}

// Portable Contracts Select Optional Codecs And Streams, Never A Storage Backend.
fn contract_facility_gate(path: &str, attribute: &str) -> bool {
    let key: String = attribute.chars().filter(|c| !c.is_whitespace()).collect();
    let owners: &[&str] = match key.as_str() {
        "#[cfg(all(feature=\"grpc\",feature=\"streaming\"))]" => {
            &["crates/harmos-contract/src/operations/serving/owned.rs"]
        }
        "#[cfg(all(feature=\"packaging\",not(target_arch=\"wasm32\")))]" => {
            &["crates/harmos-sidecar/src/lib.rs"]
        }
        "#[cfg(all(test,feature=\"derive\"))]" => &[
            "crates/harmos-contract/src/operations/authored.rs",
            "crates/harmos-contract/src/operations/serving/streams.rs",
        ],
        "#[cfg(all(test,feature=\"grpc\"))]" => &["crates/harmos/src/operations/native.rs"],
        "#[cfg(any())]" => &[
            "crates/harmos-macros/src/operation.rs",
            "crates/harmos/src/operations/authored.rs",
        ],
        "#[cfg(any(feature=\"load\",feature=\"guest-host\",feature=\"sidecar-host\"))]" => {
            &["crates/harmos/src/operations/session.rs"]
        }
        "#[cfg(feature=\"grpc\")]" => &[
            "crates/harmos-contract/src/lib.rs",
            "crates/harmos-contract/src/message/mod.rs",
            "crates/harmos-contract/src/operations/contract.rs",
            "crates/harmos-contract/src/operations/grpc.rs",
            "crates/harmos-contract/src/operations/host/payloads.rs",
            "crates/harmos-contract/src/operations/mod.rs",
            "crates/harmos-contract/src/operations/serving.rs",
            "crates/harmos-contract/src/operations/serving/owned.rs",
            "crates/harmos-contract/src/operations/serving/streams.rs",
            "crates/harmos-contract/src/operations/streaming.rs",
            "crates/harmos-contract/src/schema.rs",
            "crates/harmos-guest/examples/operation_component/contract.rs",
            "crates/harmos/src/operations/mod.rs",
            "crates/harmos/src/operations/native.rs",
            "crates/harmos/src/operations/registration.rs",
        ],
        "#[cfg(feature=\"reflection\")]" => &["crates/harmos-contract/src/lib.rs"],
        "#[cfg(feature=\"streaming\")]" => &[
            "crates/harmos-contract/src/operations/mod.rs",
            "crates/harmos-contract/src/operations/serving.rs",
            "crates/harmos-contract/src/operations/serving/owned.rs",
            "crates/harmos-contract/src/operations/serving/protobuf.rs",
        ],
        "#[cfg(not(feature=\"grpc\"))]" => &[
            "crates/harmos/src/lib.rs",
            "crates/harmos-contract/src/lib.rs",
            "crates/harmos-contract/src/observers.rs",
            "crates/harmos-contract/src/operations/serving/owned.rs",
        ],
        "#[cfg(not(feature=\"streaming\"))]" => &[
            "crates/harmos-contract/src/operations/serving.rs",
            "crates/harmos-contract/src/operations/serving/protobuf.rs",
        ],
        "#[cfg(not(target_arch=\"wasm32\"))]" => &["crates/harmos-contract/src/grpc/export.rs"],
        _ => return false,
    };
    owners
        .iter()
        .any(|owner| path.ends_with(&format!("/{owner}")))
}

// Storage Always Uses Postcard. Only Explicit Protobuf Projections And The
// Explicit Legacy Migration Entry May Be Gated; Default Codec And Engine
// Declarations Remain Subject To The Unconditional Durability Census.
fn codec_facility_gate(path: &str, attribute: &str, following: &str) -> bool {
    let key: String = attribute.chars().filter(|c| !c.is_whitespace()).collect();
    let declaration = following.trim_start();
    if path.ends_with("/crates/harmos/src/storage/catalog.rs") {
        return key == "#[cfg(feature=\"grpc\")]"
            && (declaration.starts_with("pub fn migrate_protobuf<")
                || declaration.starts_with("#[test]"));
    }
    let owners = [
        "codec.rs",
        "codec/timestamp.rs",
        "data/codec.rs",
        "files/codec.rs",
        "object_fields/codec.rs",
        "object_values.rs",
        "object_values/codec.rs",
        "operations/object_result.rs",
        "operations/object_selection.rs",
        "operations/query_storage.rs",
        "operations/selection.rs",
        "position/codec.rs",
        "schema_input/fields.rs",
    ];
    if !owners
        .iter()
        .any(|owner| path.ends_with(&format!("/crates/harmos/src/storage/{owner}")))
    {
        return false;
    }
    if key == "#[cfg(all(test,feature=\"grpc\"))]" {
        return path.ends_with("/storage/position/codec.rs")
            && declaration.starts_with("#[derive(crate::__private::prost::Message)]");
    }
    if key == "#[cfg(not(feature=\"grpc\"))]" {
        return path.ends_with("/storage/codec.rs")
            && (declaration.starts_with("pub(crate) trait Native:")
                || declaration.starts_with("impl<T: PostcardValue"));
    }
    if key != "#[cfg(feature=\"grpc\")]" {
        return false;
    }
    if declaration.starts_with("use ")
        || declaration.starts_with("impl crate::__private::Protobuf for ")
        || declaration.starts_with("#[test]")
        || declaration.starts_with("#[derive(Clone, PartialEq, prost::")
    {
        return true;
    }
    if path.ends_with("/storage/codec.rs")
        && (declaration.starts_with("pub(crate) trait Native:")
            || declaration.starts_with("impl<T: PostcardValue + crate::__private::Protobuf"))
    {
        return true;
    }
    // These Helpers Belong Exclusively To The Explicit Protobuf Projection.
    if (path.ends_with("/storage/object_fields/codec.rs")
        && declaration.starts_with("impl ObjectScalar {"))
        || (path.ends_with("/storage/schema_input/fields.rs")
            && declaration.starts_with("impl Node<'_> {"))
        || (path.ends_with("/storage/position/codec.rs")
            && declaration.starts_with("const _: () = {"))
    {
        return true;
    }
    let function = declaration
        .strip_prefix("pub(crate) ")
        .unwrap_or(declaration);
    let Some(function) = function.strip_prefix("fn ") else {
        return false;
    };
    let name = function.split(['(', '<']).next().unwrap_or_default();
    matches!(
        name,
        "write"
            | "read"
            | "size"
            | "memory"
            | "present"
            | "length_delimited_size"
            | "hash"
            | "write_metadata"
            | "bounded_count"
            | "scalar"
            | "scan"
            | "bytes_field"
            | "field_len"
            | "identifier"
            | "text_field"
            | "uint_field"
            | "write_bytes"
            | "write_cursor"
            | "write_header"
            | "write_history"
    )
}

// Exact Transport Owners Keep Facility Selection Out Of Persistence Paths.
fn transport_facility_gate(path: &str, attribute: &str) -> bool {
    let key: String = attribute.chars().filter(|c| !c.is_whitespace()).collect();
    let owners: &[&str] = match key.as_str() {
        "#[cfg(feature=\"grpc\")]" => &[
            "crates/harmos-sidecar/build.rs",
            "crates/harmos-sidecar/src/lib.rs",
            "crates/harmos-sidecar/src/wire.rs",
            "crates/harmos-sidecar/src/kit/carrier.rs",
            "crates/harmos-sidecar/src/kit/connection.rs",
            "crates/harmos-sidecar/src/kit/mod.rs",
            "crates/harmos-sidecar/src/kit/plugin.rs",
            "crates/harmos/src/lib.rs",
            "crates/harmos/src/operations/streaming.rs",
            // Trusted Context And Listener Ownership Are Transport Facilities.
            "crates/harmos/src/operations/caller.rs",
            "crates/harmos/src/operations/context.rs",
            "crates/harmos/src/operations/manager.rs",
            "crates/harmos/src/runtime.rs",
            "crates/harmos/src/runtime/lifecycle.rs",
            "crates/harmos/src/services/sidecar/carrier.rs",
        ],
        // TLS Selects The Listener's Handshake, Never Storage Or Recovery.
        "#[cfg(feature=\"grpc-tls\")]" => &[
            "crates/harmos/src/grpc/ingress.rs",
            "crates/harmos/src/grpc/listener.rs",
            "crates/harmos/src/grpc/manager.rs",
        ],
        // gRPC-Web Selects The Listener's Protocol Translation, Never Storage.
        "#[cfg(feature=\"grpc-web\")]" | "#[cfg(not(feature=\"grpc-web\"))]" => {
            &["crates/harmos/src/grpc/listener.rs"]
        }
        "#[cfg(not(feature=\"grpc\"))]" => &[
            "crates/harmos-sidecar/src/wire.rs",
            "crates/harmos-sidecar/src/kit/plugin.rs",
            "crates/harmos/src/services/sidecar/carrier.rs",
        ],
        "#[cfg(all(feature=\"kit\",feature=\"grpc\"))]" => &["crates/harmos-sidecar/src/lib.rs"],
        "#[cfg(all(feature=\"sidecar\",feature=\"grpc\"))]" => &["crates/harmos/src/lib.rs"],
        "#[cfg(feature=\"kit\")]" => &[
            "crates/harmos-sidecar/src/attach/control.rs",
            "crates/harmos-sidecar/src/attach/windows.rs",
            "crates/harmos-sidecar/src/wire.rs",
        ],
        "#[cfg(any(feature=\"kit\",test))]"
        | "#[cfg(any(feature=\"grpc\",feature=\"kit\",test))]" => {
            &["crates/harmos-sidecar/src/wire.rs"]
        }
        "#[cfg(unix)]" => &[
            "crates/harmos-sidecar/src/attach.rs",
            "examples/minibench/cli/src/commands/serve.rs",
            "examples/minischematic/cli/src/commands/serve.rs",
        ],
        "#[cfg(windows)]" => &[
            "examples/minibench/cli/src/commands/serve.rs",
            "examples/minischematic/cli/src/commands/serve.rs",
        ],
        "#[cfg(any(unix,windows))]" => &[
            "crates/harmos-sidecar/src/attach.rs",
            "crates/harmos-sidecar/src/kit/plugin.rs",
        ],
        "#[cfg(all(feature=\"sidecar-host\",feature=\"sidecar\"))]"
        | "#[cfg(not(all(feature=\"sidecar-host\",feature=\"sidecar\")))]" => {
            &["crates/harmos/benches/streams/main.rs"]
        }
        _ => return false,
    };
    owners
        .iter()
        .any(|owner| path.ends_with(&format!("/{owner}")))
}

/* === Tests === */

#[test]
fn contract_scenario() {
    {
        scenario_runtime::check("gates_preserve_persistence", || {
            let codec = "/crates/harmos/src/storage/codec.rs";
            let gate = "#[cfg(feature = \"grpc\")]";
            assert!(codec_facility_gate(
                codec,
                gate,
                "impl crate::__private::Protobuf for Value {"
            ));
            for declaration in [
                "impl crate::Message for Value {",
                "impl crate::__private::PostcardValue for Value {",
                "fn postcard_write() {}",
                "fn persist() {}",
                "struct Backend;",
            ] {
                assert!(!codec_facility_gate(codec, gate, declaration));
            }
            assert!(!codec_facility_gate(
                "/crates/harmos/src/storage/engine.rs",
                gate,
                "fn write() {}"
            ));
            assert!(!contract_facility_gate(codec, gate));

            let grpc = "#[cfg(feature = \"grpc\")]";
            assert!(transport_facility_gate("/crates/harmos/src/lib.rs", grpc));
            assert!(transport_facility_gate(
                "/crates/harmos-sidecar/build.rs",
                grpc
            ));
            assert!(transport_facility_gate(
                "/crates/harmos/src/operations/streaming.rs",
                grpc
            ));
            assert!(operation_facility_gate(
                "/crates/harmos/src/grpc/envelope.rs",
                "#[cfg(feature = \"runtime\")]"
            ));
            assert!(operation_facility_gate(
                "/crates/harmos/src/grpc/server.rs",
                "#[cfg(any(feature = \"load\", feature = \"guest-host\", feature = \"sidecar-host\"))]"
            ));
            let runtime = "#[cfg(feature = \"runtime\")]";
            assert!(operation_facility_gate(
                "/crates/harmos/src/storage/operations/save.rs",
                runtime
            ));
            for owner in [
                "storage/manager.rs",
                "storage/worker.rs",
                "storage/operations/unreviewed.rs",
            ] {
                assert!(!operation_facility_gate(
                    &format!("/crates/harmos/src/{owner}"),
                    runtime
                ));
            }
            for owner in [
                "storage/catalog.rs",
                "storage/manager.rs",
                "storage/operations/save.rs",
                "storage/data.rs",
                "state/manager.rs",
            ] {
                let path = format!("/crates/harmos/src/{owner}");
                for gate in [
                    grpc,
                    "#[cfg(feature = \"grpc-tls\")]",
                    "#[cfg(feature = \"persistence\")]",
                    "#[cfg(all(feature = \"grpc\", feature = \"persistence\"))]",
                    "#[cfg(unix)]",
                ] {
                    assert!(!transport_facility_gate(&path, gate), "{path}: {gate}");
                    assert!(!operation_facility_gate(&path, gate), "{path}: {gate}");
                }
            }
            // Runtime Shutdown Closes Transport Ingress For Both Storage Modes.
            let lifecycle = "/crates/harmos/src/runtime/lifecycle.rs";
            assert!(transport_facility_gate(lifecycle, grpc));
            for gate in [
                "#[cfg(feature = \"persistence\")]",
                "#[cfg(all(feature = \"grpc\", feature = \"persistence\"))]",
                "#[cfg(feature = \"grpc-tls\")]",
                "#[cfg(unix)]",
            ] {
                assert!(!transport_facility_gate(lifecycle, gate));
                assert!(!operation_facility_gate(lifecycle, gate));
            }
            assert!(!transport_facility_gate(
                "/crates/harmos/src/storage/carrier.rs",
                grpc
            ));
            assert!(!transport_facility_gate(
                "/examples/unreviewed/cli/src/commands/serve.rs",
                "#[cfg(unix)]"
            ));
        });
    }
    {
        scenario_runtime::check("examples_keep_contract", || {
            // The direction is one way and the lockfile is where it is checked: contract
            // below app below shells, contract below plugins, and nothing at all depending
            // on a plugin. A plugin that reached the app crate would be compiling the
            // host half of the application it is supposed to be a leaf of, and a shell
            // that reached a plugin would be linking a leaf in order to drive it.
            for (contract, application, shell, plugin) in [
                (
                    "minibench-contract",
                    "minibench-app",
                    "minibench-cli",
                    "minibench-probe-sidecar",
                ),
                (
                    "minischematic-contract",
                    "minischematic-app",
                    "minischematic-cli",
                    "minischematic-plugin",
                ),
            ] {
                let beneath = locked_dependencies(contract)
                    .expect("the contract crate is a workspace member");
                assert!(!beneath.contains(application));
                assert!(!beneath.contains(shell));
                assert!(!beneath.contains(plugin));

                let host =
                    locked_dependencies(application).expect("the app crate is a workspace member");
                assert!(host.contains(contract));
                assert!(!host.contains(shell));
                assert!(!host.contains(plugin));

                let above = locked_dependencies(shell).expect("the shell is a workspace member");
                assert!(above.contains(application));
                assert!(!above.contains(plugin));

                let leaf = locked_dependencies(plugin).expect("the plugin is a workspace member");
                assert!(leaf.contains(contract));
                assert!(!leaf.contains(application));
            }
        });
    }
    {
        scenario_runtime::check("journal_ac007_crate", || {
            probe::folds(support::Mirror::new().0);
            let _ = probe::HISTORY_REFUSALS;

            let (surface, hidden) = halves(&read(&source().join("lib.rs")));
            let handed_out: BTreeSet<String> = reexported(&surface);
            let accepted: BTreeSet<String> = CENSUS
                .iter()
                .chain(&AUTHORING)
                .map(|name| (*name).to_owned())
                .chain(["SimulatedRuntime".to_owned()])
                .collect();

            assert_eq!(handed_out, accepted);
            assert_eq!(
                reexported(&hidden),
                HIDDEN
                    .iter()
                    .map(|name| (*name).to_owned())
                    .collect::<BTreeSet<String>>(),
                "the hidden authoring module drifted"
            );
        });
    }
    {
        scenario_runtime::check("guest_kit_hands", || {
            let handed_out = reexported(&read(&workspace().join("crates/harmos-guest/src/lib.rs")));
            let accepted: BTreeSet<String> =
                GUEST_CENSUS.iter().map(|name| (*name).to_owned()).collect();

            assert_eq!(handed_out, accepted, "the guest kit's names drifted");
        });
    }
    {
        scenario_runtime::check("exactly_own_surface", || {
            let handed_out =
                reexported(&read(&workspace().join("crates/harmos-sidecar/src/lib.rs")));
            let accepted: BTreeSet<String> = SIDECAR_KIT_CENSUS
                .iter()
                .map(|name| (*name).to_owned())
                .collect();

            assert_eq!(handed_out, accepted, "the sidecar kit's names drifted");
        });
    }
    {
        scenario_runtime::check("journal_r017_deleted", || {
            let deleted: BTreeSet<&str> = DELETED.into_iter().collect();
            let declarations = declarations();
            let found: Vec<String> = declarations
                .iter()
                .filter(|(_, declared)| deleted.contains(declared.name.as_str()))
                .map(|(path, declared)| format!("{path}: {} {}", declared.keyword, declared.name))
                .collect();

            assert!(found.is_empty(), "deleted names came back: {found:?}");

            let runtime_state: Vec<String> = declarations
                .into_iter()
                .filter(|(path, declared)| {
                    path.ends_with("/crates/harmos/src/runtime.rs")
                        && declared.public
                        && declared.keyword == "fn"
                        && declared.name == "state"
                })
                .map(|(path, declared)| format!("{path}: {} {}", declared.keyword, declared.name))
                .collect();

            assert!(
                runtime_state.is_empty(),
                "the Runtime::state shortcut came back: {runtime_state:?}"
            );
        });
    }
    {
        scenario_runtime::check("missing_public_refused", || {
            let found: Vec<String> = declarations()
                .into_iter()
                .filter(|(_, declared)| declared.public)
                .filter(|(_, declared)| UNSPEAKABLE.iter().any(|noun| declared.name.contains(noun)))
                .map(|(path, declared)| format!("{path}: {}", declared.name))
                .collect();

            assert!(found.is_empty(), "public mechanism vocabulary: {found:?}");

            // Primary durability accepts either the current or an explicit data cursor;
            // local recovery remains a separate boundary. Keep these operations on
            // StorageManager; journal entry waits and stream sealing retain their own
            // contracts. New aliases or placement elsewhere fail this audit.
            let saved: Vec<String> = declarations()
                .into_iter()
                .filter(|(path, _)| !path.contains("/examples/jsonl-adapter/"))
                .filter(|(_, declared)| declared.public && declared.keyword == "fn")
                .filter(|(path, declared)| {
                    !(declared.name == "seal" && path.contains("/crates/harmos/src/streams/"))
                })
                .filter(|(_, declared)| {
                    ["save", "flush", "persist", "checkpoint", "seal"]
                        .iter()
                        .any(|verb| {
                            declared.name == *verb || declared.name.starts_with(&format!("{verb}_"))
                        })
                })
                .map(|(path, declared)| format!("{path}: {}", declared.name))
                .collect();

            let manager =
                format!("{}/src/storage/manager.rs", env!("CARGO_MANIFEST_DIR")).replace('\\', "/");
            // Retry-Protected Save Is Another Method On The Same StorageManager,
            // Keeping Its Snapshot And Durable Outcome In The Original Worker.
            let saving =
                format!("{}/src/storage/saving.rs", env!("CARGO_MANIFEST_DIR")).replace('\\', "/");
            let expected = vec![
                format!("{manager}: flush"),
                format!("{manager}: flush_until"),
                format!("{manager}: flush_recovery"),
                format!("{saving}: save_once"),
            ];
            assert_eq!(
                saved, expected,
                "only the storage manager exposes data and recovery durability boundaries"
            );
        });
    }
    {
        scenario_runtime::check("domain_keeps_runtime", || {
            let downstream: BTreeSet<&str> = DOWNSTREAM.into_iter().collect();
            let mut found = Vec::new();

            for (path, text) in sources() {
                if !path.contains("/harmos/src/state/") {
                    continue;
                }
                let code = code(&text);
                for token in identifiers(&code) {
                    // Logical Recovery Owns Boot Refusals And Bounded Storage Reads.
                    // Other State Slices Still Cannot Depend On Runtime Facilities.
                    let recovery_contract = (token == "Incompatible"
                        && ["recovery.rs", "snapshot.rs", "snapshot_codec.rs"]
                            .iter()
                            .any(|file| path.ends_with(&format!("/state/{file}"))))
                        || (token == "ReadLimits" && path.ends_with("/state/recovery.rs"));
                    if downstream.contains(token.as_str()) && !recovery_contract {
                        found.push(format!("{path}: {token}"));
                    }
                }
                for module in [
                    "crate::runtime",
                    "crate::consumer",
                    "crate::attachments",
                    "crate::evidence",
                    "crate::storage::manager",
                    "crate::storage::backend",
                    "crate::storage::worker",
                ] {
                    if code.contains(module) {
                        found.push(format!("{path}: {module}"));
                    }
                }
            }

            assert!(
                found.is_empty(),
                "state uses storage contracts without owning runtime or backend machinery: {found:?}"
            );
        });
    }
    {
        scenario_runtime::check("runtime_r015_dependency", || {
            let mut audited = 0;

            for (name, allowed) in ALLOWANCE {
                let Some(locked) = locked_dependencies(name) else {
                    continue;
                };
                let allowed: BTreeSet<&str> = allowed.iter().copied().collect();
                let outside: Vec<&String> = locked
                    .iter()
                    .filter(|dependency| !allowed.contains(dependency.as_str()))
                    .collect();

                audited += 1;
                assert!(
                    outside.is_empty(),
                    "{name} depends on more than the allowance: {outside:?}"
                );
            }

            assert!(audited >= 2, "the audit found no crate to check");
        });
    }
    {
        scenario_runtime::check("durability_lacks_compilation", || {
            // Memory-only and persisted runtimes share one capture, storage and recovery
            // path. Exact facility gates below select authoring, optional transports,
            // plugin authority, simulation execution or platform APIs. They never select
            // a persistence implementation. Audit complete attributes so multiline cfgs
            // remain subject to the same allowance as single-line attributes.
            let gated: Vec<String> = sources()
        .into_iter()
        .flat_map(|(path, text)| {
            let code = code(&text);
            code.match_indices("#[cfg(")
                .map(|(at, _)| {
                    let end = code[at..]
                        .find(")]")
                        .expect("a cfg attribute closes")
                        + at
                        + 2;
                    (code[at..end].to_owned(), &code[end..])
                })
                .filter(|(line, following)| {
                    !line.starts_with("#[cfg(test)]")
                        && !contract_facility_gate(&path, line)
                        && !codec_facility_gate(&path, line, following)
                        // Fourier Dependency Selection Never Changes Runtime Durability.
                        && !(line == "#[cfg(feature = \"fft\")]"
                            && (path.ends_with("/crates/harmos-signal/src/lib.rs")
                                || path.ends_with("/crates/harmos-signal/src/spectrum/mod.rs")))
                        && !operation_facility_gate(&path, line)
                        && !transport_facility_gate(&path, line)
                        && !((line == "#[cfg(feature = \"runtime\")]"
                            || line == "#[cfg(feature = \"storage\")]"
                            || line
                                == "#[cfg(any(feature = \"load\", feature = \"guest-host\", feature = \"sidecar-host\"))]")
                            && path.ends_with("/crates/harmos/src/lib.rs"))
                        && !(line == "#[cfg(all(not(target_arch = \"wasm32\"), feature = \"plugin\"))]"
                            && path.ends_with("/crates/harmos/src/lib.rs"))
                        // Console flags and SystemRoot belong to native process
                        // startup, not to memory/persistence selection.
                        && !(line == "#[cfg(windows)]"
                            && path.ends_with("/crates/harmos-sidecar/src/attach.rs"))
                        && !((line == "#[cfg(unix)]" || line == "#[cfg(not(unix))]")
                            && path.ends_with("/crates/harmos/src/plugin/archive.rs"))
                        && !((line == "#[cfg(feature = \"sim\")]"
                            || line == "#[cfg(not(feature = \"sim\"))]")
                            && (path.ends_with("/crates/harmos/src/lib.rs")
                                || path.ends_with("/crates/harmos/src/runtime.rs")
                                || path.ends_with("/crates/harmos/src/supervision/executor.rs")))
                        && !(line == "#[cfg(feature = \"sidecar-host\")]"
                            && (path.ends_with("/crates/harmos/src/lib.rs")
                                || path.ends_with("/crates/harmos/src/runtime.rs")
                                || path.ends_with("/crates/harmos/src/services/mod.rs")
                                || path.ends_with("/crates/harmos/src/observers/mod.rs")
                                || path.ends_with("/crates/harmos/src/load.rs")
                                || path.ends_with("/crates/harmos/src/streams/manager.rs")
                                || path.ends_with("/crates/harmos/src/streams/window.rs")))
                        // Source-loss input exists only at the sidecar seam;
                        // aggregation and retention themselves are unconditional.
                        && !(line == "#[cfg(any(feature = \"sidecar-host\", test))]"
                            && path.ends_with("/crates/harmos/src/streams/batches.rs"))
                        && !(line == "#[cfg(feature = \"guest\")]"
                            && path.ends_with("/crates/harmos/src/load.rs"))
                        // The entry crate's facade gates. Each one decides
                        // whether an application may name a facility, not
                        // whether the runtime persists — `harmos::guest` splits
                        // on the same rule one level down, so the encoder that
                        // builds a component compiles without the runtime that
                        // runs one.
                        && !((line == "#[cfg(feature = \"artifact\")]"
                            || line == "#[cfg(feature = \"sidecar\")]"
                            || line == "#[cfg(any(feature = \"sidecar\", feature = \"sidecar-host\"))]"
                            || line == "#[cfg(feature = \"guest\")]"
                            || line == "#[cfg(feature = \"signal\")]"
                            || line == "#[cfg(feature = \"fft\")]"
                            || line == "#[cfg(any(feature = \"guest\", feature = \"sidecar-host\"))]"
                            || line
                                == "#[cfg(any(feature = \"componentize\", feature = \"guest\"))]")
                            && path.ends_with("/crates/harmos/src/lib.rs"))
                        && !((line == "#[cfg(feature = \"guest\")]"
                            || line
                                == "#[cfg(all(not(target_arch = \"wasm32\"), feature = \"guest-host\"))]")
                            && path.ends_with("/crates/harmos/src/guest.rs"))
                        // Authoring binds the world and reading a built
                        // component's declaration does not, so the guest kit is
                        // its own feature and a host that only installs
                        // artifacts links no bindings generator. Which half a
                        // build takes is not a durability decision either.
                        // Transport selects the process boundary without its
                        // authoring kit; neither gate selects durable storage.
                        && !(line == "#[cfg(feature = \"transport\")]"
                            && (path.ends_with("/crates/harmos-sidecar/src/lib.rs")
                                || path.ends_with("/crates/harmos-sidecar/build.rs")))
                        && !(line == "#[cfg(feature = \"kit\")]"
                            && (path.ends_with("/crates/harmos-guest/src/lib.rs")
                                || path.ends_with("/crates/harmos-sidecar/src/lib.rs")
                                || path.ends_with("/crates/harmos-sidecar/src/error.rs")
                                || path.ends_with("/crates/harmos-sidecar/build.rs")))
                        // The authoring attributes, on by default the way serde
                        // ships its derives. What it decides is whether a build
                        // links a proc-macro chain at all — every trait beneath
                        // the attributes stays open, so a crate with this off
                        // writes the same implementations by hand. Not a
                        // durability decision by any reading.
                        && !(line == "#[cfg(feature = \"derive\")]"
                            && (path.ends_with("/crates/harmos/src/lib.rs")
                                || path.ends_with("/crates/harmos-contract/src/lib.rs")
                                || path.ends_with("/crates/harmos-guest/src/lib.rs")
                                || path.ends_with("/crates/harmos-sidecar/src/lib.rs")))
                        // Series analysis opens a read-only view over saved rows;
                        // the measurement kernels it calls are an opt-in hand-out,
                        // not a durability decision. Writes never pass through it.
                        && !(line == "#[cfg(feature = \"signal\")]"
                            && path.ends_with("/crates/harmos/src/storage/mod.rs"))
                        && !(line == "#[cfg(feature = \"fft\")]"
                            && path.ends_with("/crates/harmos/src/series/transform.rs"))
                        // Reflection selects a standard row decoder; capture and
                        // durable storage still use the same Postcard bytes.
                        && !(line == "#[cfg(feature = \"reflection\")]"
                            && path.ends_with("/crates/harmos-contract/src/schema.rs"))
                        && !(line == "#[cfg(all(test, feature = \"derive\"))]"
                            && path.ends_with("/crates/harmos-contract/src/schema/reflection.rs"))
                        && !((line == "#[cfg(feature = \"egress\")]"
                            || line == "#[cfg(not(feature = \"egress\"))]")
                            && (path.ends_with("/crates/harmos-guest/src/kit/mod.rs")
                                || path.ends_with("/crates/harmos-guest/src/lib.rs")))
                        // The example applications' one remaining gate, and it
                        // is not a durability decision: the simulation module is
                        // the same opt-in `harmos` carries one level up. There
                        // is no target gate left to exempt — an example's contract
                        // crate compiles for both targets unconditionally and its
                        // app crate compiles for the host only, which is the
                        // whole point of splitting them.
                        && !(line == "#[cfg(feature = \"sim\")]"
                            && (path.ends_with("/examples/minischematic/app/src/app.rs")
                                || path.ends_with("/examples/minibench/app/src/app.rs")))
                        && !((line == "#[cfg(not(target_arch = \"wasm32\"))]"
                            || line
                                == "#[cfg(all(not(target_arch = \"wasm32\"), feature = \"componentize\"))]")
                            && (path.ends_with("/crates/harmos-guest/src/lib.rs")
                                || path.ends_with("/crates/harmos/src/guest.rs")))
                })
                .map(|(line, _)| format!("{path}: {line}"))
                .collect::<Vec<_>>()
        })
        .collect();

            assert!(
                gated.is_empty(),
                "memory-only and persisted runtimes share one code path: {gated:?}"
            );
        });
    }
    {
        scenario_runtime::check("sim_ac005_opt", || {
            let manifest = read(
                &source()
                    .parent()
                    .expect("src has a crate root")
                    .join("Cargo.toml"),
            );
            let root = read(&source().join("lib.rs"));

            // A simulation recovers through an in-memory copy, so it is a host that
            // persists: `runtime` includes storage, and the two test-only tokio
            // features are the rest of it.
            assert!(
                manifest.contains("sim = [\"runtime\", \"tokio/macros\", \"tokio/test-util\"]")
            );
            assert!(root.contains("#[cfg(feature = \"sim\")]\npub mod sim;"));

            #[cfg(feature = "sim")]
            {
                let _ = std::any::TypeId::of::<sim_probe::Harness>();
                let _ = std::any::TypeId::of::<sim_probe::FaultDelivery>();
                let _ = std::any::TypeId::of::<sim_probe::ResidentTrace>();
                let _draw: fn() -> u64 = sim_probe::draw;
            }
        });
    }
}
