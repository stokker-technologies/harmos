---
name: harmos-architecture
description: Design Harmos application boundaries, operation ownership, resources, and runtime composition.
---
<!-- Generated from .agents/skills/harmos-architecture/SKILL.md; edit .agents/skills/harmos-architecture/SKILL.md and run `stkr project update` -->

# Harmos Architecture

Project contracts take precedence over generic architecture defaults. Read current source for exact signatures.

## Application And Contract Layout

- `contract/` owns shared message types, operation declarations and portable
  catalogs used by the application and plugins. Because guests compile it to
  WebAssembly, it depends on `harmos-contract` rather than the host facade;
  default declarations need no Tokio, Tonic, Prost or host engine. Enable `grpc`
  for explicit protobuf codecs, `reflection` for schema export, and `streaming`
  only where portable endpoint execution is needed.
- `app/` owns application state, resource-bound implementations and `app.rs`
  composition. CLI and GUI crates are shells. Plugins depend on the contract,
  never on the application runtime crate.
- Operations are the protocol. Declare `grpc(package = "...", service = "...",
  method = "...")` on the operation; do not prescribe separate consumer
  `grpc/` or `protocol/` crates/folders, hand-authored protobuf, or duplicate RPC
  handlers. Harmos's internal `grpc/` module owns transport implementation.
- Start small: `state.rs` for replayable application data, `resources.rs` for
  shared live dependencies, `operations/mod.rs` and vertical operation leaves.
  Introduce `state/` only when several cohesive state types or rules justify it;
  the runtime's internal state domain does not require consumers to copy it.
- Operations perform managed attempts, services own resident work, and observers
  own periodic production. A bounded subscription operation can stay open under
  an explicit policy and lane; it adapts an existing producer rather than owning
  a duplicate producer or scheduler.


## Catalogs And Domain Ownership

- Name domain catalogs by their role: `Operations`, `Observers`, `Services`,
  `Listeners`, `Streams`, `Records`, `Transactions`, and `Lanes`. Generic typed
  catalogs may use the application's own domain vocabulary.
- Declare each operation on its owning unit struct in a vertical leaf file.
  Catalog entries reference the leaf type; never attach operation metadata to
  enum entries.
- Assemble each catalog once and compose it directly. Avoid parallel aliases and
  catalog-wrapper functions. Use `()` for an absent generic catalog.
- Place managers inside their domains, such as `services/manager.rs` and
  `storage/manager.rs`, re-exported by the domain's `mod.rs`. There is one
  app-wide owner per domain; plugin scopes do not create independent managers.
- Keep listener delivery within stream/record ownership; no separate
  `ListenerManager`. Keep shared supervision private rather than duplicating
  service and observer lifecycle engines.
- Standard embedded, native and guest hosts use `Runtime.plugin` and
  `Handle.plugin`, the same `PluginHost` allocation. Registration is inert;
  explicit initialization activates the dependency graph. Runtime shutdown
  retires plugins while their operation/state infrastructure remains available.
  Rust-only embedded catalogs use `register_embedded` / `reload_embedded`;
  portable embedded catalogs use `register_embedded_wire` /
  `reload_embedded_wire`, projecting original contracts from resource-bound
  handlers through `AuthoredContracts`. Both use that same lifecycle/registry;
  neither requires a separate SDK host or changes original caller authority.
  Keep custom `PluginLifecycle` drivers behind the lower-level `PluginManager`
  extension rather than requiring them in ordinary applications.
- Plugin-specific public types carry the `Plugin` prefix. Manager operations use
  `harmos::Result` and `harmos::Error`, not separate per-manager error facades.


## Features And Imports

The host runtime, managed state, streams, and authoring are unconditional.
Features select optional facilities: `plugin`, `plugin-sidecar`, `plugin-guest`,
`grpc`, `reflection`, `signal`, `signal-fft`, `simulation`, and host-independent
`packaging`. Sidecar and guest hosting imply `plugin`; `grpc` implies
`reflection`; FFT implies `signal`. TLS and Web behavior is configured through
the single gRPC manager's settings.

Consumers depend on `harmos` alone. The facade exposes qualified namespaces, not
flattened aliases: `operations`, `message`, `schema`, `artifacts`, `catalog`,
`listeners`, `config`, `plugin` (host plugin management merged with the contract's
plugin declarations), `sidecar` (`plugin-sidecar`), `guest` (`plugin-guest`),
`bundle` (`plugin` or `packaging`), `builtin`, `signal` (`signal`) and `grpc`
(descriptor projection with `reflection`, the server with `grpc`). Native plugins
author through `harmos::sidecar`; build scripts project descriptors through
`harmos::grpc` with `reflection`. `__private` holds only what generated code
reaches. Crates compiled to WebAssembly (shared declaration crates and guest
components) are the one exception: they must build without the host runtime, so
they depend on `harmos-contract` and `harmos-guest`.

Each configurable subsystem owns its settings and validation in its own
`config.rs` (for example `storage/config.rs`). Root `config.rs` composes those
module-owned types without duplicate definitions. Keep at most two configuration
levels within each subsystem; root composition adds one outer level. Prefix
storage settings types with `Storage`.

Consumers explicitly supply resource and operational policy without implicit
`Default` implementations or deserialization fallbacks. Consumers own product
defaults, machine-aware sizing and operator overrides. Harmos owns protocol
constants, correctness constraints and private algorithm choices. Existing APIs
require cohesive migration with their consumers; this policy does not claim the
migration is already implemented. Managers receive only relevant settings;
live resources and mutable state remain with their domain owners.

`harmos::prelude` is a bounded authoring convenience for application entry points,
common operation/transaction traits, and macros. Domain modules retain canonical
ownership. Do not expand the prelude into an automatic manager/type inventory.

## Aggregate Vocabulary

Harmos owns one statistic vocabulary and the math behind it; consumers alias it
and never redefine a subset. `harmos::Aggregate` (from `harmos-contract`, wasm-clean,
serde snake case, `name()`/`parse()` as `count`, `mean`, `p95`) is
`Count | Sum | Mean | Median | Min | Max | Rms | Std | Percentile(u8)`. `Std` is
the population deviation; `Percentile` takes a whole percentage 0..=100 and
interpolates linearly between ranks.

The math lives in `harmos-signal`, which names nothing of Harmos so it can lift
out unchanged. `Series` answers every member exactly over a finished run;
`Summary` is the streaming, mergeable form and answers only the members where
`Aggregate::mergeable()` holds, refusing median and percentiles with `None`.
Both skip non-finite values; empty input answers `Count` and `Sum` with zero and
everything else with `None`. The `Aggregates` trait maps the vocabulary onto both:

```rust
use harmos::Aggregate;
use harmos::signal::{Aggregates, Series, Summary};

let series = Series::new(&samples); // or `samples.into_iter().collect()`
let p95 = series.aggregate(Aggregate::Percentile(95));
let settled = series.all_within(4.75, 5.25);
let merged = Summary::of(&left).merge(&Summary::of(&right));
let mean = merged.aggregate(Aggregate::Mean);
```

Every reduction path evaluates through `Series` or `Summary`: plot buckets
(`streams::Bucket::aggregate`), stream batch measurements
(`streams::Measurement::aggregate`) and storage group-bys
(`storage::reduce::Agg::Of(Aggregate)`). Storage additionally keeps `Agg::Len`
and `Agg::Last`, which select rows rather than compute statistics, and answers
`Sum`, `Mean` and `Median` of integer and exact columns in exact rational
arithmetic. Do not add a private statistic implementation beside them.

## Simulation Surface

Keep the shared builder at `harmos::Builder`. Supporting simulation names belong
under `harmos::simulation`; do not expose internal reboot factories or restore obsolete
runtime aliases in consumer APIs.

For concrete plugin and transport authoring, use `harmos-authoring`.

## Storage Ownership And Queries

StorageManager owns read/write admission, accounting, persistence and generic
retention. Consumer applications supply explicit StorageSettings from
storage/config.rs; root config.rs composes those settings. Do not reintroduce
library-selected operational defaults or report-specific read allowances.
StorageRequestSettings controls queue/dispatch capacity; StorageResponseSettings
batch_rows and batch_bytes control delivery ceilings, independently of query
row offset/limit. Preserve bounds on in-flight writes and encoding scratch as
well as pending descriptions and data.

Query is a declarative storage-owned plan with a full StorageCursor, a selected
source, ordered transforms and a final row slice. The cursor identifies the
requested history; it does not itself pin retention. Admission protects the
required data for the read lifetime. A pruned or foreign history errors rather
than silently selecting a newer boundary. Sources cover objects, records,
streams, schemas and finished-file metadata. ReadFile is separate and reads
finished immutable content; explicit producing/follow/resume APIs retain their
separate lifecycle.

Use Arrow Schema and RecordBatch for analytical results. Emit one schema event,
then bounded batches, then the operation's final completion response. Native
calls use the same contract without forced serialization. Wire codecs must
preflight allocation bounds, preserve schema equality and use the existing
operation transport. Shared Arrow allocations remain charged through clones
and slices until their last owner releases them. A consumer-created deep copy
is the consumer's allocation; it must not be confused with a free zero-copy
escape from storage accounting.

Keep one storage transform catalog and the shared statistic implementation.
Validate input/output schemas before emitting results; apply transforms in
order and row slicing last. Aggregate full selected data; downsample display
traces only. Preserve exact integer/rational semantics where supported. Do not
introduce arbitrary code execution, a second query engine or browser-side
evidence calculations to bypass the shared contract.

Separate typed write operations share acceptance/durability machinery. Each
logical batch validates and commits atomically; transport frames do not define
commit boundaries. Accepted work belongs to storage and cannot be withdrawn by
cancelling its operation. Durable receipts prove persistence. Connection loss
can leave partial committed success and unknown unacknowledged outcomes; do not
blindly replay uncertain batches. Long-lived execution writers must not inherit
a short fixed operation lifetime. Consumer acquisition policy may drop unsent
measurement data, but storage must not silently discard accepted work or
managed-state/control transitions.

StateManager owns transactions, live state and replay. StateBackend stores the
journal and snapshots through StorageManager. Keep state journal positions
separate from evidence cursors. Required evidence must become durable before
state referring to it is replayable; snapshots must be durable before covered
journal entries can be reclaimed. Do not imply atomicity across independent
backends merely because acknowledgments mention both.

Consumers own lifecycle retention policy, including preserving all revisions
needed by active executions. Generic retention releases must survive ordinary
persistence, replication and bootstrap. Subject sealing cannot race active
materializations or accepted late writes. Report finalization, stop behavior,
renderer resources and stopped-report evidence boundaries belong to Dokime,
not Harmos. Temporary contention may wait; impossible working sets or retained
result deadlocks must fail explicitly rather than loop or omit results.
