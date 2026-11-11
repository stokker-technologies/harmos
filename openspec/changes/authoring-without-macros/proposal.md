# Authoring Without Macros

Status: agreed with the owner on 2026-11-10; implementation is authorized. The source
inventory is recorded below; declaration replacement follows the authoritative host
protobuf cutover. Extends `grpc-plugins`, which already removes the plugin, observer
and catalog macros for plugin authors, to host-side and application macros.

## Why

`harmos-macros` still generates host, data and guest declarations. No declaration it serves
needs an attribute macro. Only reading a type's fields justifies generated code, and a derive does that.
Everything else is data an application can build as ordinary values, composed and passed by
value, and checked when it starts.

## What Changes

Derives stay for data. Each attribute macro becomes a derive plus a value, in this order, from
the cheapest and safest to the riskiest:

| Step | Macro | Becomes |
| --- | --- | --- |
| 1 | `domain_` and `runtime_` catalog/operation aliases | Inventory live consumers; migrate with steps 7–8 before deleting exports |
| 2 | `listener` | A typed listener value; its callback is looked up at start |
| 3 | `streams` | A list of stream values |
| 4 | `lanes` | Lane values in a set that checks its capacities when built |
| 5 | `record` | A typed record entry over a schema derive |
| 6 | `stream` | A derive that reads the row's fields, plus a stream value |
| 7 | `operation` | A derive on the request, plus `Operation::new(id, function)` |
| 8 | `catalog` | A catalog value that lists operations |
| 9 | `records` | Typed record entries in an owned registry |
| 10 | `message` | A derive |
| 11 | `transaction`, `transactions` | A typed entry per transaction in a state catalog |

```rust
#[derive(harmos::Message, Clone, Debug, PartialEq)]
pub struct ListDirectories {}

let list = Operation::new("schematic.list.directories", list_directories)
    .policy(POLICY)
    .visibility(Visibility::Public);
let catalog = Catalog::new().operation(list).build()?;
application.operations(catalog);
```

```rust
// Transaction and Invertible implementations stay as authored.
let movement = TransactionEntry::<Schematic, MoveSymbol>::new("symbol.move").reversible::<MoveSymbol>();
let changes = StateCatalog::new().transaction(movement).build()?;
application.state(changes);
```

The `domain_catalog`, `runtime_catalog`, `domain_operation` and `runtime_operation` aliases
are live through domain/runtime/operations re-exports and their consumers. Step 1 inventories
and schedules their migration; it does not delete live aliases. Replace them with their
operation/catalog families at steps 7–8, then remove their exports and generators.

After step 11, consolidate surviving field-reading derives in `sdk/rust/derive`
(`harmos-sdk-derive`) and delete `harmos-macros`, coordinated with `grpc-plugins` 6.10.
Keep stored-data `Described`, protobuf Message and row field access semantically distinct;
the SDK must gain no host dependencies. The `grpc-plugins` order is authoritative host
protobuf, this change's ordered replacements and derive consolidation, domain reduction,
then guests with 7.1. Guest builder registration and deletion of `harmos-guest-macros`
belong to that deferred guest lifecycle/SDK adapter migration.

## What Is Given Up

- An operation that calls an operation or uses a resource it did not declare fails when the call
  happens, not at compile time.
- Duplicate identifiers, invalid lane capacities and a missing listener callback are reported
  when the application starts.
- Typed catalog enums no longer give compile-time membership proofs; code that needs a handle
  asks the registry for it and gets an error if it is absent.

## State Management

Step 11 is last because state is the feature applications build on and the migration with the
most risk. The transaction macros give an author stable stored identities, automatic schema
revisions, restoration across versions, replay and inverse routing for undo. The replacement
keeps all of that in library code and costs two to four registration lines per transaction.
Before step 11 is implemented, the replay, recovery and inverse-law tests pass unchanged against
the new registry, and a transaction with no inverse is declared as a barrier explicitly, never
assumed.

## Impact

- Breaking for every application and plugin: about 230 operations, 230 catalogs, 110
  transactions and records, and 840 messages in Dokime alone. Dokime's own macro crate depends on
  several of these and migrates in the same delivery.
- Schema revisions keep being computed from the fields; no hand-maintained version is introduced.

## Source Inventory

This inventory covers attribute sites in Harmos source, examples, test fixtures and
quoted generator tests. Counts are textual sites, including macro_rules templates;
they are not expanded declaration counts. Domain, runtime and guest aliases are
grouped with their operation/catalog family. External application adoption remains
the Workflow Follow-Up in tasks.md and is outside this worktree's ownership.

The host-protobuf cutover replaces built-in and native example request/result declarations
with generated messages. Core operations no longer has a `runtime` feature. Private guest
operation/catalog attributes moved to `crates/harmos-guest/src/builtins/` and
`examples/schematic/operations/src/guest/`; they remain owned by the deferred guest path.
The inventory below reflects these moves and is locked for implementation. External
application adoption remains a follow-up outside this repository task.

### Projections and Replacement Owners

| Family | Existing Projections to Migrate Together |
| --- | --- |
| `listener` | `listeners::Listener`, typed `Delivery<L>`/`Callback<L>`, `Member<L>` for listener entries, guest listener inventory and startup callback validation |
| `streams` | `StreamCatalog<Row>` membership, `catalog()`/`definition(id)`, inert catalog projections, `Application::Streams` and runtime stream registration |
| `lanes` | `Lane` enum/default/name/capacity/catalog, `Application::Lanes`, registration and call-context lane parameters, launch configuration and admission |
| `record` | Stable `Identified::ID`, schema-derived revision, restoration, `Record`, metadata and catalog membership |
| `stream` | Stored row identity, field access/row metadata, `StreamSpec`/`StreamRow`, default window and stream definition |
| `operation` | Request codecs/schema, `OperationDeclaration`, policy/visibility/gRPC mapping, resource/callee declarations, `Operation` dispatch and registration |
| `catalog` | `Catalog`/`InertCatalog`, visitor projections, wire declarations/callees, operation entries/registration, requirements and typed membership |
| `records` | `RecordCatalog`, typed projections, encoding/restoration and storage/runtime registration |
| `message` | Native stored schema, Postcard codec/decoding-memory accounting, `MemorySize`, row metadata, Debug/Default handling and nested/container field semantics |
| `transaction`, `transactions` | Stable identities/revisions, preparation, restoration, replay, inverse routing/mutual pairing, explicit barriers, catalog dispatch and journal recovery |

`Described` continues to describe stored Serde values. The SDK's existing
`Message` derive describes protobuf values. Consolidation must preserve that
separation and keep the SDK's product dependency graph limited to protocol.

### Live Alias Exports

`crates/harmos-domain/src/lib.rs` re-exports `domain_catalog` and `domain_operation`
as `catalog` and `operation`. Its consumers include storage tests and the schematic
domain. `crates/harmos-operations/src/execution/mod.rs`,
`crates/harmos-runtime/src/lib.rs` and `crates/harmos-runtime/src/runtime.rs`
re-export the runtime aliases used by native handlers, host test catalogs and the
schematic operation/application consumers below. None of these aliases is dead.
Keep them until their whole family is replaced in tasks 3.1 and 3.2.

The guest aliases are also live through `crates/harmos-guest/src/lib.rs` and
`crates/harmos-runtime/src/guest/mod.rs`, including guest lifecycle operations,
the sandbox fixture and schematic guest dependencies. Guest registration remains
deferred; its macro crate may accept explicit constant description values needed
by the migrated consumers in this workspace.

### Manifest, Fixture and Guest Ownership

Deleting `harmos-macros` in task 4.3 includes edits to root
`Cargo.toml` (the workspace dependency), `Cargo.lock` and
`tools/python/src/harmos_tools/build/fixtures.py`. The fixture producer's
`LIBRARIES` still names `harmos_macros`, and its library resolver special-cases
that procedural-macro artifact. Removing the crate without migrating this producer
breaks the required Tests artifact. These paths belong to this worktree's owner.

`crates/harmos-guest-macros/src/artifact.rs` also consumes const catalog projections:
`InertCatalog<listeners::Declarations, listeners::Entry>::ENTRIES` for listeners,
plus operation declaration/callee trees and stream entries. Its listener consumer
is the single `listener` attribute site in `tests/application/macros.rs` below.
Replacing all catalog consumers with owned registries must account for this guest
contract while guest macros remain deferred. The owner authorizes changes to that
crate needed for constant descriptions; explicit constant entry slices preserve
guest artifact inspection without host dependencies or declaring macros.

### Attribute Sites

| File | Families (Textual Sites) |
| --- | --- |
| `crates/harmos-domain/src/aggregate.rs` | `message` (2) |
| `crates/harmos-domain/src/schema/json.rs` | `message` (4) |
| `crates/harmos-domain/tests/message/aggregate.rs` | `message` (3) |
| `crates/harmos-domain/tests/message/allocation.rs` | `message` (8) |
| `crates/harmos-domain/tests/message/boxed.rs` | `message` (5) |
| `crates/harmos-domain/tests/message/enums.rs` | `message` (9) |
| `crates/harmos-domain/tests/message/main.rs` | `message` (15) |
| `crates/harmos-domain/tests/message/maps.rs` | `message` (5) |
| `crates/harmos-domain/tests/message/native.rs` | `message` (6) |
| `crates/harmos-guest/src/builtins/storage.rs` | `operation` (1) |
| `crates/harmos-guest/src/builtins/streams.rs` | `operation` (1) |
| `crates/harmos-guest/src/dispatch/registration_tests.rs` | `operation` (1) |
| `crates/harmos-guest/src/lifecycle/initialize.rs` | `operation` (1) |
| `crates/harmos-guest/src/lifecycle/shutdown.rs` | `operation` (1) |
| `crates/harmos-guest/tests/fixtures/sandbox/src/contract.rs` | `message` (4) |
| `crates/harmos-guest/tests/system/plugin.rs` | `message` (3) |
| `crates/harmos-macros/src/catalog/mod.rs` | `transactions` (1) |
| `crates/harmos-macros/src/catalog/operation.rs` | `operation` (1) |
| `crates/harmos-macros/tests/catalog/main.rs` | `operation` (1) |
| `crates/harmos-macros/tests/fixtures/bad_operation_id.rs` | `operation` (1) |
| `crates/harmos-macros/tests/fixtures/boxed_container_field.rs` | `message` (2) |
| `crates/harmos-macros/tests/fixtures/boxed_scalar_field.rs` | `message` (2) |
| `crates/harmos-macros/tests/fixtures/capacitated_default_lane.rs` | `lanes` (1) |
| `crates/harmos-macros/tests/fixtures/defaultless_lanes.rs` | `lanes` (1) |
| `crates/harmos-macros/tests/fixtures/doubly_defaulted_lanes.rs` | `lanes` (1) |
| `crates/harmos-macros/tests/fixtures/duplicate_id.rs` | `stream` (2) |
| `crates/harmos-macros/tests/fixtures/facade/lib.rs` | `stream` (1), `message` (1) |
| `crates/harmos-macros/tests/fixtures/facade_catalog_with_override.rs` | `catalog` (2), `operation` (2), `message` (1) |
| `crates/harmos-macros/tests/fixtures/facade_catalog_without_override.rs` | `catalog` (2), `operation` (2), `message` (1) |
| `crates/harmos-macros/tests/fixtures/facade_lanes_with_override.rs` | `lanes` (1) |
| `crates/harmos-macros/tests/fixtures/facade_lanes_without_override.rs` | `lanes` (1) |
| `crates/harmos-macros/tests/fixtures/facade_message_with_override.rs` | `stream` (2), `message` (6) |
| `crates/harmos-macros/tests/fixtures/facade_message_without_override.rs` | `message` (1) |
| `crates/harmos-macros/tests/fixtures/facade_operations_with_override.rs` | `catalog` (1), `operation` (1), `message` (1) |
| `crates/harmos-macros/tests/fixtures/facade_operations_without_override.rs` | `catalog` (1), `operation` (1), `message` (1) |
| `crates/harmos-macros/tests/fixtures/facade_runtime_catalogs.rs` | `catalog` (2), `operation` (2) |
| `crates/harmos-macros/tests/fixtures/feature_catalog.rs` | `catalog` (1), `operation` (2) |
| `crates/harmos-macros/tests/fixtures/malformed_harmos_path.rs` | `stream` (1) |
| `crates/harmos-macros/tests/fixtures/nested_box_field.rs` | `message` (2) |
| `crates/harmos-macros/tests/fixtures/non_serde_row.rs` | `stream` (1) |
| `crates/harmos-macros/tests/fixtures/operation_on_impl.rs` | `operation` (1) |
| `crates/harmos-macros/tests/fixtures/operation_on_variant.rs` | `catalog` (1), `operation` (1) |
| `crates/harmos-macros/tests/fixtures/optional_box_field.rs` | `message` (2) |
| `crates/harmos-macros/tests/fixtures/repeated_box_field.rs` | `message` (2) |
| `crates/harmos-macros/tests/fixtures/shared_message_field.rs` | `message` (3) |
| `crates/harmos-macros/tests/fixtures/uncatalogued_inverse.rs` | `transaction` (2), `transactions` (1) |
| `crates/harmos-macros/tests/fixtures/undeclared_operation.rs` | `operation` (2) |
| `crates/harmos-macros/tests/fixtures/undeclared_resource.rs` | `operation` (1) |
| `crates/harmos-macros/tests/fixtures/unpaired_inverse.rs` | `transaction` (2) |
| `crates/harmos-macros/tests/fixtures/unregistered_row.rs` | `lanes` (1) |
| `crates/harmos-macros/tests/fixtures/unsafe_id.rs` | `stream` (1) |
| `crates/harmos-macros/tests/fixtures/zero_window.rs` | `stream` (1) |
| `crates/harmos-operations/src/execution/grpc/mod.rs` | `message` (2) |
| `crates/harmos-operations/src/execution/grpc/transcode.rs` | `message` (4) |
| `crates/harmos-operations/src/execution/operations/operation.rs` | `operation` (2) |
| `crates/harmos-packaging/tests/acceptance/bindings.rs` | `catalog` (1), `operation` (4), `message` (2) |
| `crates/harmos-packaging/tests/support/proto_contracts.rs` | `catalog` (3), `operation` (6), `message` (2) |
| `crates/harmos-runtime/benches/operations/main.rs` | `catalog` (1), `operation` (1), `lanes` (1) |
| `crates/harmos-runtime/benches/sidecar/fixture.rs` | `lanes` (1) |
| `crates/harmos-runtime/src/clock.rs` | `operation` (1), `lanes` (1) |
| `crates/harmos-runtime/src/grpc/codec.rs` | `message` (1) |
| `crates/harmos-runtime/src/grpc/envelope.rs` | `message` (1) |
| `crates/harmos-runtime/src/guest/collection.rs` | `lanes` (1) |
| `crates/harmos-runtime/src/guest/host/plugin/scope.rs` | `operation` (1), `lanes` (1) |
| `crates/harmos-runtime/src/operations/invocation.rs` | `operation` (9), `lanes` (1), `message` (1) |
| `crates/harmos-runtime/src/operations/surface.rs` | `operation` (1), `lanes` (1) |
| `crates/harmos-runtime/src/operations/wire.rs` | `operation` (1), `lanes` (1) |
| `crates/harmos-runtime/src/services/mod.rs` | `lanes` (1) |
| `crates/harmos-runtime/src/state/history.rs` | `lanes` (1), `record` (1) |
| `crates/harmos-runtime/src/state/recovery.rs` | `lanes` (1) |
| `crates/harmos-runtime/tests/application/macros.rs` | `catalog` (4), `operation` (2), `lanes` (2), `listener` (1), `stream` (1), `record` (3), `records` (2), `transaction` (4), `transactions` (2), `message` (1) |
| `crates/harmos-runtime/tests/fixtures/benchmark_protocol.rs` | `catalog` (1), `operation` (1), `lanes` (1), `message` (1) |
| `crates/harmos-runtime/tests/integration/report.rs` | `message` (1) |
| `crates/harmos-runtime/tests/operations/admission.rs` | `catalog` (1), `operation` (1) |
| `crates/harmos-runtime/tests/operations/cancellation.rs` | `operation` (2) |
| `crates/harmos-runtime/tests/operations/causation.rs` | `catalog` (1), `operation` (3) |
| `crates/harmos-runtime/tests/operations/children.rs` | `catalog` (1), `operation` (2), `message` (1) |
| `crates/harmos-runtime/tests/operations/clock.rs` | `catalog` (1), `operation` (1), `message` (1) |
| `crates/harmos-runtime/tests/operations/host.rs` | `operation` (2) |
| `crates/harmos-runtime/tests/operations/lanes.rs` | `catalog` (1), `operation` (1), `lanes` (1) |
| `crates/harmos-runtime/tests/operations/middleware.rs` | `operation` (5), `message` (1) |
| `crates/harmos-runtime/tests/operations/outcomes.rs` | `catalog` (1), `operation` (2), `message` (1) |
| `crates/harmos-runtime/tests/operations/policy.rs` | `operation` (6) |
| `crates/harmos-runtime/tests/operations/resources.rs` | `catalog` (1), `operation` (2), `message` (2) |
| `crates/harmos-runtime/tests/operations/scoped.rs` | `catalog` (1), `operation` (3), `message` (1) |
| `crates/harmos-runtime/tests/operations/shutdown.rs` | `catalog` (1), `operation` (1), `message` (1) |
| `crates/harmos-runtime/tests/operations/wire/artifact.rs` | `catalog` (3), `operation` (2), `message` (1) |
| `crates/harmos-runtime/tests/operations/wire/authored.rs` | `catalog` (1), `operation` (5), `message` (2) |
| `crates/harmos-runtime/tests/operations/wire/authored_lifecycle.rs` | `operation` (2) |
| `crates/harmos-runtime/tests/operations/wire/cases.rs` | `catalog` (3), `operation` (6), `message` (1) |
| `crates/harmos-runtime/tests/operations/wire/dynamic.rs` | `operation` (2), `message` (1) |
| `crates/harmos-runtime/tests/operations/wire/endpoints.rs` | `operation` (5) |
| `crates/harmos-runtime/tests/operations/wire/grpc.rs` | `operation` (1) |
| `crates/harmos-runtime/tests/operations/wire/grpc_manager.rs` | `operation` (7), `message` (2) |
| `crates/harmos-runtime/tests/operations/wire/grpc_network.rs` | `operation` (7) |
| `crates/harmos-runtime/tests/operations/wire/owned_wire.rs` | `catalog` (1), `operation` (1), `message` (1) |
| `crates/harmos-runtime/tests/plugin_grpc/external_authorization.rs` | `catalog` (1), `operation` (1), `message` (1) |
| `crates/harmos-runtime/tests/plugin_grpc/invoke_support.rs` | `message` (2) |
| `crates/harmos-runtime/tests/runtime/lifecycle/application.rs` | `catalog` (2), `operation` (3) |
| `crates/harmos-runtime/tests/runtime/lifecycle/shutdown.rs` | `catalog` (1), `operation` (1) |
| `crates/harmos-runtime/tests/services/main.rs` | `catalog` (1), `operation` (1) |
| `crates/harmos-runtime/tests/services/shutdown.rs` | `catalog` (1), `operation` (1) |
| `crates/harmos-runtime/tests/simulation/inverses.rs` | `lanes` (1) |
| `crates/harmos-runtime/tests/simulation/main.rs` | `catalog` (1), `operation` (4), `record` (1), `records` (1), `transaction` (1), `transactions` (1) |
| `crates/harmos-runtime/tests/state/capture.rs` | `lanes` (1), `record` (1), `records` (1), `transaction` (1), `transactions` (1) |
| `crates/harmos-runtime/tests/state/durability_atomic_commit.rs` | `lanes` (1), `record` (5), `records` (1), `transaction` (1), `transactions` (1) |
| `crates/harmos-runtime/tests/state/durability_wire.rs` | `lanes` (1), `records` (1), `transactions` (1) |
| `crates/harmos-runtime/tests/state/listeners.rs` | `catalog` (2), `operation` (2), `lanes` (1), `record` (1), `records` (1), `message` (1) |
| `crates/harmos-runtime/tests/state/managed.rs` | `lanes` (1) |
| `crates/harmos-runtime/tests/state/records.rs` | `lanes` (1), `record` (1), `records` (1), `message` (2) |
| `crates/harmos-runtime/tests/storage/operation_objects.rs` | `catalog` (1), `message` (1) |
| `crates/harmos-runtime/tests/storage/operation_queries.rs` | `catalog` (1), `message` (4) |
| `crates/harmos-runtime/tests/storage/state/admission.rs` | `lanes` (1), `record` (1), `records` (1), `transaction` (1), `transactions` (1) |
| `crates/harmos-runtime/tests/storage/state/capture.rs` | `lanes` (1), `record` (1), `records` (1), `transaction` (1), `transactions` (1), `message` (1) |
| `crates/harmos-runtime/tests/storage/state/composition.rs` | `message` (3) |
| `crates/harmos-runtime/tests/storage/state/managed.rs` | `lanes` (1), `record` (1), `records` (1), `transaction` (1), `transactions` (1) |
| `crates/harmos-runtime/tests/storage/state/reconcile.rs` | `message` (1) |
| `crates/harmos-runtime/tests/storage/state/shape_versions.rs` | `catalog` (3), `lanes` (1), `streams` (1), `stream` (2), `record` (3), `records` (2), `transaction` (1), `transactions` (1), `message` (3) |
| `crates/harmos-runtime/tests/storage/state/snapshots.rs` | `lanes` (1), `record` (1), `records` (1), `transaction` (1), `transactions` (1) |
| `crates/harmos-runtime/tests/streams/listeners.rs` | `catalog` (2), `operation` (2), `lanes` (1), `streams` (1), `message` (1) |
| `crates/harmos-runtime/tests/streams/main.rs` | `streams` (1), `stream` (1) |
| `crates/harmos-runtime/tests/streams/revisions.rs` | `message` (2) |
| `crates/harmos-runtime/tests/streams/streams_macros.rs` | `lanes` (1), `streams` (1), `stream` (3) |
| `crates/harmos-runtime/tests/support/allocation.rs` | `lanes` (1), `streams` (1), `stream` (3) |
| `crates/harmos-runtime/tests/support/durability.rs` | `lanes` (1), `record` (1), `records` (1), `transaction` (5), `transactions` (1) |
| `crates/harmos-runtime/tests/support/mod.rs` | `lanes` (1), `streams` (1), `stream` (2), `message` (1) |
| `crates/harmos-runtime/tests/support/native_streams.rs` | `lanes` (1), `streams` (1), `stream` (1), `message` (1) |
| `crates/harmos-runtime/tests/support/operations_fixture.rs` | `catalog` (2), `operation` (3) |
| `crates/harmos-runtime/tests/support/runtime_probe.rs` | `lanes` (1) |
| `crates/harmos-runtime/tests/system/guest.rs` | `lanes` (1) |
| `crates/harmos-runtime/tests/system/guest_contract/dependencies.rs` | `catalog` (2), `operation` (2) |
| `crates/harmos-runtime/tests/system/guest_contract/lifecycle.rs` | `catalog` (4), `operation` (2) |
| `crates/harmos-runtime/tests/system/runtime.rs` | `message` (1) |
| `crates/harmos-storage/benches/query/source/main.rs` | `message` (1) |
| `crates/harmos-storage/src/query/source.rs` | `message` (3) |
| `crates/harmos-storage/tests/integration/manager/batching.rs` | `message` (1) |
| `crates/harmos-storage/tests/integration/manager/mapping.rs` | `message` (4) |
| `crates/harmos-storage/tests/integration/manager/objects.rs` | `catalog` (4), `message` (4) |
| `crates/harmos-storage/tests/support/waveforms.rs` | `catalog` (1), `message` (2) |
| `examples/schematic/app/src/lanes.rs` | `lanes` (1) |
| `examples/schematic/app/src/records/export.rs` | `record` (1) |
| `examples/schematic/app/src/records/mod.rs` | `records` (1) |
| `examples/schematic/app/src/streams/mod.rs` | `streams` (1) |
| `examples/schematic/app/src/streams/voltage.rs` | `stream` (1) |
| `examples/schematic/app/src/transactions/mod.rs` | `transactions` (1) |
| `examples/schematic/app/src/transactions/net/apply.rs` | `transaction` (1) |
| `examples/schematic/app/src/transactions/net/connect.rs` | `transaction` (1) |
| `examples/schematic/app/src/transactions/net/rename.rs` | `transaction` (1) |
| `examples/schematic/app/src/transactions/symbol/movement.rs` | `transaction` (1) |
| `examples/schematic/app/src/transactions/symbol/place.rs` | `transaction` (1) |
| `examples/schematic/domain/src/model/schematic/mod.rs` | `message` (3) |
| `examples/schematic/domain/src/model/schematic/progress.rs` | `stream` (1), `message` (1) |
| `examples/schematic/domain/src/model/schematic/validated.rs` | `catalog` (1), `message` (1) |
| `examples/schematic/domain/src/model/transfer.rs` | `message` (4) |
| `examples/schematic/operations/src/guest.rs` | `catalog` (1) |
| `examples/schematic/operations/src/guest/schematic/analyze.rs` | `message` (2), `operation` (1) |
| `examples/schematic/operations/src/guest/schematic/propose.rs` | `operation` (1), `message` (1) |
| `examples/schematic/operations/src/guest/schematic/validate.rs` | `operation` (1), `message` (1) |
| `examples/schematic/operations/src/guest/transfer/download.rs` | `operation` (1) |
| `examples/schematic/operations/src/guest/transfer/hash.rs` | `operation` (1), `message` (1) |
| `examples/schematic/operations/src/guest/transfer/mod.rs` | `catalog` (2) |
| `examples/schematic/operations/src/guest/transfer/put.rs` | `operation` (1) |
| `examples/schematic/operations/src/guest/transfer/take.rs` | `operation` (1) |
| `examples/schematic/operations/src/guest/transfer/upload.rs` | `operation` (1) |
| `examples/schematic/storage/tests/journal/main.rs` | `lanes` (1), `record` (5), `records` (1), `transaction` (1), `transactions` (1) |
| `examples/schematic/storage/tests/streams/main.rs` | `lanes` (1), `streams` (1), `stream` (1) |
| `examples/schematic/storage/tests/support/mod.rs` | `lanes` (1), `record` (1), `records` (1), `transaction` (5), `transactions` (1) |
