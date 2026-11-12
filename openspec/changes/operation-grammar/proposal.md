Status: locked by the owner on 2026-11-12
Boundary, managers and nested resources locked by the owner on 2026-11-12
Naming rule amended by the owner on 2026-11-12: bare verbs on the service's resource
Separation by audience locked by the owner on 2026-11-12
Three visibility levels locked by the owner on 2026-11-12

# Operation Grammar

## Why

Operation names, paging, retry policies and generated transports currently express
the same concepts differently across the library and its consumers. A shared
grammar makes migration predictable while preserving distinct durability, atomic
mutation, file lifecycle and cooperative cancellation guarantees.

## What Changes

This document records direction, not landed behavior or permission to implement.
“Owner” means locked by the owner on 2026-11-12. Requirement deltas describe
the locked contract; implementation is not authorized by this documentation task.

| Decision | Status | Direction |
| --- | --- | --- |
| Audience Separation | LOCKED, 2026-11-12 | Each service has one audience face and package: harmos.plugin.v1.Instance (host calls plugin), harmos.context.v1 (plugin child calls host), harmos.v1 and parent sub-packages (clients call host). Shared capabilities have separate face-local request/response contracts, even with a shared handler. Plugins remains client entry management. |
| Visibility | LOCKED, 2026-11-12 | Explicit Private (plugin-internal, SDK-local, absent from catalog and every wire, no gRPC binding), Protected (plugin and host, catalogued, host-to-plugin carrier only, no client gRPC binding), or Public (other plugins by declared requirement and clients, through the host; client gRPC binding permitted). No SDK default. Faces do not replace visibility. Built-in mapping is decided in design.md. |
| Face Generation | Owner | Plugin SDK: serve protocol and call context; client: client API only; host: all three. Exported Protected/Public plugin operations stay catalog operations carried by Instance.Invoke in their plugin face, never client services; Private operations remain local. |
| Names | Owner | Resource services with bare verbs; single-mode parts use nouns; multimode parts are nested resource services with bare verbs; mixed-resource Storage retains its resource nouns. Service-qualified wrappers follow design.md. |
| Reply envelopes | Owner | Always return the response wrapper, including empty results, rather than a bare resource. |
| Ordinary verbs | Owner | Closed set: Get, List, Watch, Set, Create, Delete, Append. Update, Read, Browse, Put, Edit, Add, Remove, Follow, Scan, Inspect and Register are not verbs in the new grammar. |
| Immutable ordered data | Owner | Use Append for records, samples, log lines and upload bytes. |
| Set | Owner | Whole writable value plus the expected revision returned by the matching Get; stale revisions conflict with the action “get again and retry”. Split large writable parts into separately settable sub-resources with their own Get/Set. |
| Domain actions | Owner | Use the explicitly defined domain verbs only where a standard verb would hide a side effect; retain Sleep for the clock wait. |
| Position-resolved reads | Owner | One read path per kind of data, optional `at`, absent means latest, reply reports `resolved_at`; paging and watches share the rule. Accept only previously returned positions for continuation or the product's terminal read selection, not arbitrary earlier positions. |
| Latest selection | Owner | Resolve latest durable data without waiting. Durable lag must stay within 250 ms at p95, independent of payload size. A caller requiring its own write passes its receipt's durable position. Benchmark small, large and mixed loads; try smaller flush pieces and size-triggered flushing first if the bound is missed. Accepted-data snapshots are the named fallback, not built. |
| Cancellation | Owner | Cancel interrupts running work; queued work is refused with an error naming the action. Stop finishes at a safe boundary and remains distinct. Product scheduling policy belongs to the product. |
| Rename | Owner | Renaming changes the identified resource only, never sibling attempts of its parent. This is application behavior; no library rename RPC is invented. |
| Transfers | Owner | One unary, ranged, resumable mechanism for clients and plugins: Create session, Append ordered bytes at offsets, Get status, Publish content, Delete to abandon; downloads use repeated bounded Get ranges returning total length and digest. Configuration bounds chunks and ranges. Measure large-file throughput in Nightly before deleting streaming transfers. |
| Streaming gRPC methods | Owner | Never generate streaming twins automatically. Streams are reserved for Watch and plugin protocol carriers; use unary resource actions and Get/Watch; retain one managed stream only for an explicitly promised cooperative final result, replacing its ordinary gRPC method. |
| Failed client calls | Owner | Resending is the client's responsibility; the same request identifier and normalized payload return the original receipt, and a changed payload under that identifier conflicts. |
| Shared primitives | Owner | One paging, position, time, Failure, retry-policy and receipt vocabulary, as specified in design.md. |
| SDK ownership | Owner | Other products will also host plugins on this library: language SDKs stay here and product SDKs keep building on them. Protocol, SDKs and conformance remain self-sufficient without product vocabulary. Client roots expose package-derived resource hierarchies in all four languages; plugin semantic parity and generic managed-client invocation scope remain open. |
| Library Boundary | LOCKED, 2026-11-12 | Generic plugin hosting/entry management, access, transfers, host status, operations/invocations, time, storage, streams, protocol and SDKs belong to the library, under their owning managers. A product serves these services directly by composition. |
| Product Boundary | LOCKED, 2026-11-12 | A product defines its storage schemas, indexes, projections and latest selection, objects, object operations and authorization policy. Settings/secrets stores, schemas, UI declarations, archives and operating-system credential integration remain product-owned. |
| Plugin Values | LOCKED, 2026-11-12 | The library owns only generic host-value delivery to plugins and the settings-declaration section of a plugin catalog; a product resolves settings/secrets. The per-plugin settings/secrets ownership question is closed. |
| No Duplicate Machinery | LOCKED, 2026-11-12 | A product must compose the runtime rather than rebuild on the protocol crate alone: doing so previously duplicated plugin connection, dispatch, streams and process launch. |
| Managers | LOCKED, 2026-11-12 | Apply the live-ownership rule and library manager inventory in design.md; plain storage operations need no manager. |
| Nested Resources | LOCKED, 2026-11-12 | Use parent sub-packages and bare part-service verbs, including cross-library Plugins.Settings; generate hierarchical client roots mechanically from descriptors in all four languages. Host applications may declare nested resources in their own packages beside library services, without redeclaring them. |

**BREAKING:** rename the operations in the complete design inventory; merge the
three row-follow operations into WatchRows; replace streaming uploads and downloads with
the shared unary session and ranged Get reads; delete automatic `Run<Method>` gRPC methods and
their envelopes. Retain distinct backend recovery records and file-production
recovery metadata; no public checkpoint RPC family exists here today.

Preserve bounded physical scans, ordered indexes, exact-revision projections,
atomic object replacement, host-clock waits, device-driven publication,
file replacement/truncation events, writer fencing and typed cancellation results.
The proposal neither grants new external access nor removes capabilities merely
because inspected callers do not use them.

## Capabilities

### New Capabilities

- `plugin-management`: owner extension on 2026-11-12; persistent managed and remote entries, verified bundle installation and the client-facing `Plugins` resource service.

### Modified Capabilities

- `operation-contract`: method grammar, wrappers, action meanings and one policy vocabulary.
- `operation-invocation`: explicit gRPC methods, cancellation and isolated metadata edits.
- `storage-visibility`: position-resolved reads, bounded paging and watches.
- `storage-mutations`: whole-value revision-guarded Set, immutable Append and durability receipts.
- `storage-transfers`: resumable unary transfers shared by all callers.

## Impact

The library gains client Plugins, Access/Access.Listener and
Status/Status.Logs/Status.TraceEvents, plus context-only HostValues. Existing
plugin-manager work is integrated, not reimplemented. The wire inventory is:

| Face | Services | Operations | gRPC Methods | Requests / Responses |
| --- | ---: | ---: | ---: | ---: |
| harmos.plugin.v1 | 1 | 4 | 4 | 4 / 4 |
| harmos.context.v1 | 6 | 40 | 40 (0 standalone) | 40 / 40 |
| harmos.v1 and nested packages | 10 (3 nested) | 58 | 58 | 58 / 58 |

Totals: 17 services, 102 face operations, 102 declared gRPC methods (62 standalone) and
102 request/response pairs. Four retained native-only storage hooks add four
request/response pairs without a wire service: 106 inventory operations and
212 wrappers overall. The full mapping and 38 capabilities declared separately
on context and client faces are in design.md; no client or plugin can bypass the
host to reach another instance. These counts describe the locked candidate only.

Future implementation touches `proto/harmos/plugin/v1/`, `proto/harmos/context/v1/`, `proto/harmos/v1/`, `crates/harmos-protocol/`,
`crates/harmos-operations/`, `crates/harmos-storage/`, runtime gRPC projection and
plugin hosting, `sdk/`, `tools/conformance/`, generators, examples and their tests.
Every renamed ID changes its fingerprint: requirements, manifests, descriptor
sets, constants, generated clients and dispatch must migrate together. Callers
must unwrap responses, retain `resolved_at`, continue with bound page tokens,
consume accepted/durable receipts, replace old follows and streaming transfers,
and select only explicitly declared managed gRPC methods.

SDK retention is decided, without an implicit commitment to four generic managed
client implementations. Correct the five independently actionable defects in
design.md within the locked grammar migration. Implementation must update
affected README and topic-guide claims with the behavior they describe; this
documentation-only change leaves current usage documentation intact.

Only this change directory is authored now. No source, protocol, test, workflow,
release, stored-data migration or specification-lifecycle activation is included.
