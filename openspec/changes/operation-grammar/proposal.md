Status: proposed, not locked

# Operation Grammar

## Why

Operation names, paging, retry policies and generated transports currently express
the same concepts differently across the library and its consumers. A shared
grammar makes migration predictable while preserving distinct durability, atomic
mutation, file lifecycle and cooperative cancellation guarantees.

## What Changes

This document records direction, not landed behavior or permission to implement.
“Owner” means decided by the owner on 2026-11-12; “proposed” means a recommendation
awaiting the owner's lock. Requirement deltas describe the candidate contract.

| Decision | Status | Direction |
| --- | --- | --- |
| Names | Owner | Symmetric, predictable `VerbResource`; `<Method>Request` and `<Method>Response`. |
| Reply envelopes | Proposed | Always return the response wrapper, including empty results, rather than a bare resource. |
| Ordinary verbs | Proposed | Closed set: Get, List, Watch, Set, Create, Delete, Append. Update, Read, Browse, Put, Edit, Add, Remove, Follow, Scan, Inspect and Register are not verbs in the new grammar. |
| Immutable ordered data | Proposed | Use Append in both repositories for records, samples, log lines and other ordered immutable data. |
| Set | Proposed | Optional field mask; absence means the whole writable value. Expected revision guards revisioned resources. |
| Domain actions | Proposed | Use the explicitly defined domain verbs only where a standard verb would hide a side effect; retain Sleep for the clock wait. |
| Position-resolved reads | Owner | One read path per kind of data, optional `at`, absent means latest, reply reports `resolved_at`; paging and watches share the rule. Accept only previously returned positions for continuation or the product's terminal read selection, not arbitrary earlier positions. |
| Latest selection | Proposed | Resolve latest durable data without a persistence barrier or an accepted-data snapshot. A caller requiring its own write passes its receipt's durable position. Measure visibility lag before locking. |
| Cancellation | Owner | Cancel interrupts running work; queued work is refused with an error naming the action. Stop finishes at a safe boundary and remains distinct. Product scheduling policy belongs to the product. |
| Rename | Owner | Renaming changes the identified resource only, never sibling attempts of its parent. This is a product behavior; no library rename RPC is invented. |
| Uploads | Proposed; recommendation accepted by owner | Use one resumable unary upload-session mechanism for clients and plugins; plugin operations remain unary. |
| Streaming routes | Proposed | Never generate streaming twins automatically. Prefer unary resource actions and Get/Watch; retain one managed stream only for an explicitly promised cooperative final result, replacing its ordinary route. |
| Shared primitives | Proposed | One paging, position, time, Failure, retry-policy and receipt vocabulary, as specified in design.md. |
| SDK ownership | Open | Whether this library keeps language SDKs or product SDKs implement the plugin protocol directly is not decided. |

**BREAKING:** rename the operations in the complete design inventory; merge the
three row-follow operations into WatchRows; replace streaming file upload with
the shared unary production session; delete automatic `Run<Method>` routes and
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
- `operation-invocation`: explicit routes, cancellation and isolated metadata edits.
- `storage-visibility`: position-resolved reads, bounded paging and watches.
- `storage-mutations`: masked Set, immutable Append and durability receipts.
- `storage-transfers`: resumable unary upload sessions shared by all callers.

## Impact

Future implementation touches `proto/harmos/v1/`, `crates/harmos-protocol/`,
`crates/harmos-operations/`, `crates/harmos-storage/`, runtime gRPC projection and
plugin hosting, `sdk/`, `tools/conformance/`, generators, examples and their tests.
Every renamed ID changes its fingerprint: requirements, manifests, descriptor
sets, constants, generated clients and dispatch must migrate together. Callers
must unwrap responses, retain `resolved_at`, continue with bound page tokens,
consume accepted/durable receipts, replace old follows and streaming uploads,
and select only explicitly declared managed routes.

SDK retention is a lock prerequisite, not an implicit commitment to four new
client implementations. Correct the five independently actionable defects in
design.md regardless of the larger grammar decision. Implementation must update
affected README and topic-guide claims with the behavior they describe; this
documentation-only change leaves current usage documentation intact.

Only this change directory is authored now. No source, protocol, test, workflow,
release, stored-data migration or specification-lifecycle activation is included.
