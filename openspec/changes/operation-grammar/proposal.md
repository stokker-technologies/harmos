Status: locked by the owner on 2026-11-12

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
| Names | Owner | Symmetric, predictable `VerbResource`; `<Method>Request` and `<Method>Response`. |
| Reply envelopes | Owner | Always return the response wrapper, including empty results, rather than a bare resource. |
| Ordinary verbs | Owner | Closed set: Get, List, Watch, Set, Create, Delete, Append. Update, Read, Browse, Put, Edit, Add, Remove, Follow, Scan, Inspect and Register are not verbs in the new grammar. |
| Immutable ordered data | Owner | Use Append in both repositories for records, samples, log lines and upload bytes. |
| Set | Owner | Whole writable value plus the expected revision returned by the matching Get; stale revisions conflict with the action “get again and retry”. Split large writable parts into separately settable sub-resources with their own Get/Set. |
| Domain actions | Owner | Use the explicitly defined domain verbs only where a standard verb would hide a side effect; retain Sleep for the clock wait. |
| Position-resolved reads | Owner | One read path per kind of data, optional `at`, absent means latest, reply reports `resolved_at`; paging and watches share the rule. Accept only previously returned positions for continuation or the product's terminal read selection, not arbitrary earlier positions. |
| Latest selection | Owner | Resolve latest durable data without waiting. Durable lag must stay within 250 ms at p95, independent of payload size. A caller requiring its own write passes its receipt's durable position. Benchmark small, large and mixed loads; try smaller flush pieces and size-triggered flushing first if the bound is missed. Accepted-data snapshots are the named fallback, not built. |
| Cancellation | Owner | Cancel interrupts running work; queued work is refused with an error naming the action. Stop finishes at a safe boundary and remains distinct. Product scheduling policy belongs to the product. |
| Rename | Owner | Renaming changes the identified resource only, never sibling attempts of its parent. This is a product behavior; no library rename RPC is invented. |
| Transfers | Owner | One unary, ranged, resumable mechanism for clients and plugins: Create session, Append ordered bytes at offsets, Get status, Publish content, Delete to abandon; downloads use repeated bounded Get ranges returning total length and digest. Configuration bounds chunks and ranges. Measure large-file throughput in Nightly before deleting streaming transfers. |
| Streaming gRPC methods | Owner | Never generate streaming twins automatically. Streams are reserved for Watch and plugin protocol carriers; use unary resource actions and Get/Watch; retain one managed stream only for an explicitly promised cooperative final result, replacing its ordinary gRPC method. |
| Shared primitives | Owner | One paging, position, time, Failure, retry-policy and receipt vocabulary, as specified in design.md. |
| SDK ownership | Owner | Other products will also host plugins on this library: language SDKs stay here and product SDKs keep building on them. Protocol, SDKs and conformance remain self-sufficient without product vocabulary. Only four-language plugin parity and generic managed-client language scope remain open under SDKs. |

**BREAKING:** rename the operations in the complete design inventory; merge the
three row-follow operations into WatchRows; replace streaming uploads and downloads with
the shared unary session and ranged Get reads; delete automatic `Run<Method>` gRPC methods and
their envelopes. Remove separate checkpoint read families in the product, while
retaining this library's distinct backend recovery records and file-production
recovery metadata. No such public checkpoint RPC family exists here today.

Preserve bounded physical scans, ordered indexes, exact-revision projections,
atomic object replacement, host-clock waits, device-driven publication,
file replacement/truncation events, writer fencing and typed cancellation results.
The proposal neither grants new external access nor removes capabilities merely
because inspected product callers do not use them.

## Capabilities

### New Capabilities

None; the candidate adds requirements to existing capability owners.

### Modified Capabilities

- `operation-contract`: method grammar, wrappers, action meanings and one policy vocabulary.
- `operation-invocation`: explicit gRPC methods, cancellation and isolated metadata edits.
- `storage-visibility`: position-resolved reads, bounded paging and watches.
- `storage-mutations`: whole-value revision-guarded Set, immutable Append and durability receipts.
- `storage-transfers`: resumable unary transfers shared by all callers.

## Impact

Future implementation touches `proto/harmos/v1/`, `crates/harmos-protocol/`,
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
