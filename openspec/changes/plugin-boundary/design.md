# Plugin Boundary Design

## Context

See proposal.md for motivation. The owner decision of 2026-11-16 follows comparison
of go-plugin, the Grafana plugin SDK, Kubernetes CSI/CRI, Wasmtime components and
OSGi. The selected approach retains a runtime and carrier with product-driven
loading. It rejects both a full library author platform and a host backend trait
that would make every product reconstruct transport semantics.

Verified in this worktree before authoring:

| Current Responsibility | Source Evidence |
| --- | --- |
| Bundle format and artifact inventory | `crates/harmos-runtime/src/bundle/manifest.rs:55` defines the manifest; `tools/packaging/src/bundle/mod.rs` builds archives. |
| Installation mixed into activation | `crates/harmos-runtime/src/plugin/manager/content.rs:48` prepares content; `:83` inspects a bundle and `:109` extracts it. |
| Persisted settings and replay receipts | `crates/harmos-runtime/src/plugin/manager/settings.rs:88` sets values; `:98` replays receipts and `:149` commits the registry. |
| Generic carrier already carries opaque bytes | `proto/harmos/plugin/v1/invocation.proto:37` carries invocation payloads; `:65` carries child payloads; `proto/harmos/model/v1/description.proto:47` carries product sections. |
| Shared renderer implementation | `tools/python/src/harmos_tools/build/resource_roots.py:77` builds descriptor tables; `:440`, `:824`, `:1185` and `:1793` locate language renderers. Compare the consuming product's implementation before moving code. |

Architecture invariants being amended: invariant 8 assigns plugin management
"entries, typed settings, placement, activation and shutdown"; invariant 9 says
"The library stores plugin settings beside entries"; invariant 17 says "Keep all
four SDKs in this repository". Their purpose is one live owner and reusable
machinery. The new split preserves that purpose while moving installed-entry
policy and authoring to products. Historical proof of those owners is not proof
of this boundary. `operation-boundary` D7 and the `plugin-lifecycle` SDK ownership
statements are amended in this delivery, not left as competing decisions.

## Goals / Non-Goals

**Goals:** one activation state machine, carrier implementation and generator
implementation reusable by several products; no product process probe or copied
window/cancellation engine; product policy exposed only through product APIs.

**Non-Goals:** no `PluginBackend`, author-facing Harmos SDK, library installation
service, product semantics in the carrier, compatibility shim or data migration.
Existing storage history, lifecycle and operation policy guarantees remain.

## Decisions

### D1. Runtime and Policy Have Distinct Owners

| Harmos | A Product |
| --- | --- |
| Internal plugin manager and live activation state | Installed-plugin registry and startup selection |
| Register → admission → initialize → finalize machinery | Product contract and four author SDKs, registrars and starters |
| Descriptor closure, identities, fingerprints, declaration digests | Semantic admission of product declarations |
| Process/Wasm hosting, transport, reconnect and fencing, operation routing | Installation, bundle format/build/verification, manifests, artifact digests and distribution |
| Sampling/window engine, deadlines and cancellation | Persisted settings and their resolution policy; management operations and projections |
| Minimal carrier, four-language helpers, descriptor/codegen tools, inspection and generic conformance | Product conformance and one self-contained contracts archive per release |

The carrier closure includes connection identity/revision, admission descriptors,
invocation correlation and opaque success/failure payloads, cancellation, host-call
frames, opaque scopes and product declaration sections, windows and sample frames.
Lifecycle control comes from `plugin-lifecycle`. Helpers retain time, own-invocation
and admitted discovery facilities over host-call frames; own settings resolve
through the product kind's hook. Product SDKs translate their public values
internally; product policy and DTOs do not enter the carrier. Harmos retains its
runtime clock, invocation and routing mechanics. Generic declaration digests identify admitted
definitions; artifact digests identify installed bytes and belong with bundles.

### D2. Product-Driven Loading Through the Typed Facade

Proposed shape, not landed symbols; it refines `operation-boundary` D6:

```rust
setup.plugin_kind("product.grpc",
    grpc::Kind::<ProductContract>::new().admit(product::admit));
let host = runtime.for_call(&ctx);
let plugin = host.plugins().load(Load {
    kind, placement: Managed { command }, settings,
}).await?;
let binding = host.plugins().bind(plugin, scope, clock).await?;
let demand = host.plugins().start_samples(binding, streams).await?;
```

The kind supplies product descriptors/codecs and admission plus settings-resolution
callbacks. They receive the inherited call context and generic-validated frozen
declarations; Harmos does not interpret product sections or open a registry.
Callbacks are bounded by the owning load operation's policy, with cancellation
propagated, rather than a second timeout/retry authority. Settings are resolved
before initialize. Product admission refuses before initialize and publication;
partial failure settles owned transport/process resources. No runtime callback
discovers installed entries or chooses which one to load.

The product calls load for its installed entries at startup. Managed placement
takes a product-selected command, remote placement an endpoint and Wasm placement
owned component bytes. Loading owns spawn/attach, verification, callbacks,
initialize and rollback; bind and sampling return opaque handles. Handles retain
activation generations and fence stale use, never expose managers or engines.
No automatic install/registry/settings backend or service is introduced.

### D3. Inspection Reuses Process Ownership

Expose bounded descriptor inspection for managed, remote and supported Wasm
placements through the facade. Inspection obtains frozen metadata and performs
generic validation without initialize, sampling, operational handlers, registry
writes or installation. It is separate from runtime contribution registration;
the old registration-only "no inspect" statement does not prohibit this facility.
The operation owns its deadline, metadata/message bounds and cancellation, using
the existing process owner, transport and fencing rather than a second probe.
An owned child is signalled, killed if necessary and reaped under existing teardown
policies even on failure. A remote endpoint is detached without terminating the
remote process. Refusals name the executable/endpoint and corrective action.
Product interpretation of inspection output stays product-owned.

### D4. One Tooling and Carrier Implementation

Consolidate the product's richer descriptor tables/renderers and fixes into
`tools/python/src/harmos_tools/build/`, parameterized by supplied descriptors,
namespace and output locations, without importing product packages. Preserve
presence, nested messages, language keywords, enum and repeated-value handling,
typed client/context projections and call options. Product tasks invoke these
tools with product inputs; they do not retain forked renderer implementations.

Rust, Python, C# and C++ helpers retain reusable cancellation, windows, transport,
guest ABI where supported, failures and connection plumbing. Products implement
their own public registrar, operation, policy, context, window and error surfaces
over these helpers. No Harmos type, namespace, attribute or escape hatch is
required by an author; qualification is internal, never an import alias solution.
Generic conformance uses a fixture product contract; product conformance covers
its author APIs and semantics. It never makes Harmos own a product kind.

### D5. One Cutover With the Other Boundary Changes

Implementation follows the six ordered task groups. Generator and inspection
consolidation precede the policy move. Bundle verification/building and registry,
settings and management operations transfer together to one product owner,
preserving CAS atomicity, receipts and explicit incompatible-root refusal. Remove
the superseded library bundle builder and management paths after consumers move;
do not leave two archive implementations. Baseline v1 and existing storage
history guarantees remain; incompatible development roots are never reset.

Lock carrier, facade, lifecycle and product declarations before one final
regeneration across `operation-boundary`, `plugin-lifecycle` and this change.
Generated output remains tracked; generation tasks own it. Products package the
carrier closure with their own protos, all imported `google/*`, licenses and
protoc/Buf instructions, checking closure during packaging. Published upstream
revisions replace integration-only overrides before final validation.

## Risks / Trade-Offs

- [Product SDKs could copy four carrier engines] → retain thin reusable helpers;
  product wrappers own the public surface and translation only.
- [Policy move loses durable behavior] → transfer atomic operations and replay
  receipts with their deterministic fault tests; refuse incompatible roots.
- [Renderer consolidation drops product fixes] → compare both implementations
  before deletion and cover descriptor behavior with focused generator fixtures.
- [Inspection leaks a child on cancellation] → use the runtime's retained owner
  and prove settlement on every refusal path with controlled events.
- [Cross-change generation churn] → one final coordinated regeneration; no
  compatibility path or manual generated edits.
