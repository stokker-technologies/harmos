# Authoring Without Macros

Status: agreed with the owner on 2026-11-10; implementation waits for the owner's go. Extends
`grpc-plugins`, which already removes the plugin, observer and catalog macros for plugin authors,
to the macros used on the host side and by applications.

## Why

`harmos-macros` is about 5,900 lines with 3,700 lines of tests. No concept it serves needs an
attribute macro. Only reading a type's fields justifies generated code, and a derive does that.
Everything else is data an application can build as ordinary values, composed and passed by
value, and checked when it starts.

## What Changes

Derives stay for data. Each attribute macro becomes a derive plus a value, in this order, from
the cheapest and safest to the riskiest:

| Step | Macro | Becomes |
| --- | --- | --- |
| 1 | Unused `domain_` and `runtime_` variants | Deleted |
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

After the change about 2,000 lines of derives remain: the portable message derive, `Described`
for stored data, and row field access.

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
