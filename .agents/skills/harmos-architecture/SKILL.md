---
name: harmos-architecture
description: Design Harmos application boundaries, operation ownership, resources, and runtime composition.
---

# Harmos Architecture

Apply `stkr-rust-architecture` for the tier model. Harmos assigns the following
crate owners; dependencies below describe production edges, not test dependencies.

## Application And Contract Layout

| Crate | Ownership | Harmos Dependencies |
| --- | --- | --- |
| `harmos` | Consumer entry library and host facade. | `harmos-domain`, `harmos-runtime`. |
| `harmos-domain` | Host-free messages, contracts, schemas and portable catalogs. | Optional `harmos-macros` for declarations. |
| `harmos-runtime` | Resource-bound execution and host composition. | `harmos-domain`, `harmos-sdk`, `harmos-storage`, `harmos-macros`; optional `harmos-guest`, `harmos-sidecar`, `harmos-packaging`. |
| `harmos-sdk` | Shared execution and plugin lifecycle contracts. | `harmos-domain`. |
| `harmos-storage` | Storage engine, Arrow queries, persistence and signal kernels. | `harmos-domain`. |
| `harmos-guest` | Wasm component SDK. | `harmos-domain`, `harmos-sdk`. |
| `harmos-sidecar` | Native process carrier and SDK. | `harmos-domain`, `harmos-sdk`. |
| `harmos-packaging` | Artifact receipts, bundles and schema/binding generation. | `harmos-domain`. |
| `harmos-macros` | Declaration and authoring macros. | No Harmos crate dependency. |
| `cargo-harmos` | Cargo subcommand for product packaging and authoring. | `harmos-packaging`. |

The library crates publish to crates.io; `cargo-harmos` ships through release executables. The SDK, storage, carrier,
packaging and macro boundaries keep their consumers independent of the larger
host runtime.

Declare a product's operation contracts, including requests and replies, records,
streams, prompts and settings models once in `<p>-domain`. Depend on
`harmos-domain` for host-free Harmos declarations. Implement handlers in
`<p>-runtime`; nothing re-declares or forwards an operation. Every shipped
interface imports the product entry library `<p>` and provides the same
operations through its own host.

## Hosted Applications

An app is a role, not a tier: an application is `<p>-runtime` hosted as a
sidecar. Its hosting binary is a `[[bin]]` of the runtime unless the company
crate split rule applies.

A resource library beside the runtime is legitimate when it wraps one external
system, such as a proprietary process's protobuf API or desktop automation.
Otherwise keep that responsibility as a module of the runtime.

## Catalogs And Domain Ownership

Catalogs assemble declarations owned by the product domain. Operation metadata
belongs to the request declaration; catalog entries reference that declaration.
Runtime handlers implement those contracts against the runtime's resources.

A plugin is a native process the host launches and supervises over piped stdio.
Every Dokime plugin is one. A sidecar uses the same carrier for a product's own
runtime. A guest is a Wasm component run in-process by the host. Dokime admits
native sidecars only.

Harmos registers sidecars and guests under the same runtime plugin owner.
Registration describes contributions; activation owns their execution.
There is no attachment path for a runtime on another machine.

Cross-provider routing lets a plugin's requirements name operations another
registered provider offers. The host resolves by contract and shared visibility
and rejects ambiguity. A provider's operation identity alone is insufficient:
the declared version, schema, policy, visibility and replay contract must match.
Explicit provider selection must still satisfy the same contract and visibility.

## Features And Imports

Use `harmos` for host composition, `harmos-domain` for portable declarations,
`harmos-sidecar` for native carrier consumers and `harmos-guest` for guests.
Both carrier crates share `harmos-sdk`; neither depends on the host runtime.

The runtime's `plugin-sidecar` and `plugin-guest` features imply `plugin`.
Its `grpc` feature owns transport composition; domain declarations stay
host-free. SDK and carrier protobuf codecs are optional `grpc` facilities.
`harmos-packaging` owns schema and binding generation independently of runtime
execution.

## Aggregate Vocabulary

`harmos-domain` owns `Aggregate`, re-exported through `harmos`:
`Count`, `Sum`, `Mean`, `Median`, `Min`, `Max`, `Rms`, `Std` and
`Percentile(u8)`. Consumers reuse that vocabulary.

`harmos-storage` owns signal mathematics. `Series` represents a finished
series; `Summary` represents mergeable streaming statistics. Median and
percentiles require the finished series. The storage `fft` feature owns FFT
kernels; the host facade exposes them through `signal-fft`.

## Simulation Surface

Simulation belongs to `harmos-runtime` and is exposed through `harmos`.
It does not introduce another product tier or another owner for contracts.

## Storage Ownership And Queries

`harmos-storage` owns read/write admission, accounting, persistence, retention
and the query engine. The runtime storage wrapper supplies the host clock and
resident-worker ownership around the same engine.

Consumers own storage budgets, admission limits, concurrency and response batch
sizes. Harmos owns protocol constants and correctness constraints. Product
settings belong to the product domain; the runtime owns the configured live
storage resources.

Storage owns the declarative `Query`, `StorageCursor`, source and transform
vocabulary. Arrow schemas and record batches are analytical results; clones and
slices retain the original storage allocation's charge. The query engine and
signal kernels remain shared owners across interfaces.

The runtime state manager owns transactions, live state and replay. Its storage
backend owns journal and snapshot persistence through the storage engine.
Evidence cursors and state journal positions describe distinct histories.
