# Authoring Without Macros

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

- Schema revisions keep being computed from the fields; no hand-maintained version is introduced.

`Described` continues to describe stored Serde values. The SDK's existing
`Message` derive describes protobuf values. Consolidation must preserve that
separation and keep the SDK's product dependency graph limited to protocol.
