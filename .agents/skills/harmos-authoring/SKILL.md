---
name: harmos-authoring
description: Author Harmos operations, plugins, streaming endpoints, and generated transport contracts.
---

# Harmos Authoring

Use `harmos-architecture` for the crate map and hosting roles. Declare contracts
once in the product domain and implement them in the product runtime.

## Resources And Operation Ownership

Keep requests, replies, records, streams, prompts and settings models in
`<p>-domain`. Keep devices, clients, file handles and execution ownership in
`<p>-runtime`. Runtime handlers implement the original domain declarations.

Use `#[harmos_domain::message]` for portable messages and
`#[harmos_domain::operation]` for operation declarations. Host-side declarations
use the same attributes through `harmos`. Put operation metadata on the request
struct; catalogs reference that struct.

```rust
#[harmos_domain::message]
#[harmos_domain::operation(
    id = "schematic.validate",
    response = Validation,
    policy = POLICY,
    version = 1,
)]
pub struct Validate {
    pub schematic: Snapshot,
}

#[harmos_domain::catalog]
pub enum Operations {
    Validate(Validate),
}
```

The request is the declaration's owning type. `response` names its final reply;
`input` and `output` declare streamed item types independently. An operation
with neither endpoint is unary; either or both directions may stream.

`policy`, `version` and visibility belong to the original declaration.
A handler binding or catalog entry does not declare a second contract.
Shared visibility admits compatible cross-provider requirements; a linked Rust
type alone grants no provider authority.

Messages describe portable values: booleans, integer and floating-point scalars,
strings, bytes, nested messages, optional values and repeated values.
`Box<T>` supports a nested message without changing its wire identity.
Reference-counted live resources do not belong in message declarations.

`Message` owns Postcard encoding in `harmos-domain`. Protobuf is a separate,
explicit codec owned by the SDK and transport facilities. A message's schema
and encoding are part of its contract.

## gRPC Composition And Discovery

Declare protobuf exposure on the original operation with
`grpc(package = "...", service = "...", method = "...")`. The runtime exposes
that contract through its gRPC host; an interface does not re-declare the
operation or add another handler for it.

```rust
#[harmos_domain::message]
#[harmos_domain::operation(
    id = "meter.read",
    response = Reading,
    policy = POLICY,
    version = 1,
    grpc(package = "meter.v1", service = "Meter", method = "Read"),
)]
pub struct Read {
    pub channel: u32,
}
```

`harmos-domain` owns finite schema graphs and operation metadata.
`harmos-packaging` owns their static schema and binding projection.
`harmos-runtime` owns discovery of currently available providers.
These are views of the same declarations, not separately authored contracts.

## Storage Reads

Storage owns `Query`, its cursor, sources and ordered transforms.
The source vocabulary covers objects, records, streams, schemas and file
metadata. Row slicing follows the selected transforms.

`ReadFile` describes finished immutable content. `FollowRecords`,
`FollowStream`, `FollowObjects` and `FollowFile` describe live subscriptions.
Product interfaces consume those storage contracts through their host.

Storage owns admitted data and its budget accounting. The runtime owns the
resident workers that keep storage active; viewers do not own producers.
Consumer settings supply operational budgets and response batch ceilings.

## Canonical Guest And Sidecar Layout

The product domain owns portable declarations shared by its runtime and
interfaces. The runtime owns handlers and external-system resources.
Its sidecar binary hosts the runtime's original operations.

A native plugin uses `harmos-sidecar`; a Wasm guest uses `harmos-guest`.
Their carrier choice does not create another contract tier.
Both use the shared SDK operation and plugin declarations.

A Dokime plugin normally embeds the product runtime through `<p>` and registers
its handler catalog as what it provides. The plugin owns only its suites,
templates, observers, listeners and its own records and streams.

When crash isolation or a native library requires the runtime's own process on
the same machine, host that runtime as a sidecar providing the same operations.
The plugin only requires those operations and depends on `<p>-domain` alone.
This is local process hosting; a runtime on another machine has no attachment
path.

## Unified Plugin Contracts

Declare provided and required operations with `#[harmos::plugin]`.
`provides` identifies the handler catalog; `requires` identifies original
operation declarations supplied by other registered providers.

```rust
#[harmos::plugin(
    id = "meter",
    provides = Operations,
    requires(acme::charger::Measure),
)]
```

The host resolves requirements by exact contract and shared visibility.
Multiple compatible providers require an unambiguous selection.
Neither a carrier choice nor a provider selection changes the operation's
request, reply or ownership.

Stream, record and listener catalogs describe contributions under their
original owners. Native observer contributions use the declared observer
catalog. Guest observer callbacks are unsupported; sharing SDK vocabulary
does not promise identical carrier capabilities.
