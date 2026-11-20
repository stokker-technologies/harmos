# Observer Collection Design

## Context

These findings describe the source before this change, not landed collection
support:

| Claim | Verified Source |
| --- | --- |
| One observer owns one stream. | `sdk/rust/src/stream.rs:50` stores one stream ID; `sdk/rust/src/plugin.rs:263` rejects multiple observers or an observer combined with a producer. |
| One-sample observers return exactly one row per sample. | `sdk/rust/src/stream.rs:147` accepts `Result<T>`; `:242` calls the handler for every granted sample and transposes those rows. |
| A whole-window authoring form already exists. | `sdk/rust/src/stream.rs:168` accepts one handler returning columns for the entire window. It still requires one value per sample in every column. |
| Unobserved interval streams do no sampling. | `crates/harmos-runtime/src/plugin/grpc/publish.rs:397` requires positive stream demand and no scoped suppression; `:657` waits for that gate before granting windows. |
| A listener does not currently own the Samples connection lifetime. | `crates/harmos-runtime/src/plugin/grpc/publish.rs:141` prepares and opens streams before their demand gates run. Resource hooks therefore cannot be attached to RPC open/close alone. |
| Handler errors fail the window. | `sdk/rust/src/server.rs:485` maps handler errors to a gRPC status; `crates/harmos-runtime/src/plugin/grpc/samples.rs:153` turns that status into `WindowFailed`. |
| Existing batches cannot express sparse or repeated rows per tick. | `proto/harmos/plugin/v1/stream.proto:36` encodes exactly one column value per sample; `crates/harmos-runtime/src/plugin/grpc/samples.rs:171` requires positive consecutive counts completing the grant. |
| Scoped clocks are independent. | `docs/architecture.md:1235` and `crates/harmos-runtime/src/plugin/grpc/publish.rs:567` retain an independent schedule for each scope and its clock. |

The reported product-side producer is outside this repository. Its location and
removal cannot be verified here.

## Goals and Boundaries

Use the existing observer/window machinery for grouped demand. Preserve scoped
clock isolation, child-call authority, controlled-clock wait settlement, overload
failures and attachment-owned teardown. Resources used by a collection belong to
active listener demand rather than process lifetime or idle transport lifetime.

No changes belong under `docs/`, `.github/` or runtime state ownership. Generated
sources remain tracked and are changed only by generation tasks.

## Proposed Decisions

These are reviewable design choices, not implemented contracts. The orchestrator
must activate the agreed change before implementation.

### Identity and Scheduling

A collection owns a nonempty, disjoint set of declared streams of one plugin.
Use the lexicographically first member stream ID as its transport identity;
derive it in the SDK, so the small authoring API needs no additional identifier.
Reject missing, duplicate or multiply owned members during declaration admission.

Group existing `PreparedDemand`/`Demand` entries by collection rather than create
a second scheduler. A singleton observer is a group of one. The collection's
constant interval or Duration setting is resolved once, by the current host
resolver. Member intervals must agree; invalid declarations name the declaration
the author must correct.

Resolve the shared window length once at activation as the shortest resolved
member window, including members currently without demand. A member without an
override uses `PlacementSettings::interval_window`. This fixed choice avoids
rescheduling when listeners overlap or change. Windows may contain several due
ticks as they do today, but the collection handler runs exactly once per tick,
never once per member stream. A short window containing no due tick invokes no
handler. A controlled advance covering N due ticks invokes the handler N times.

The schedule key remains activation, scope and bound clock. Distinct scoped
clocks cannot be merged without breaking existing scoped sampling guarantees.
Within one key, overlapping listeners share one handler invocation per tick.
Across keys, each scope gets its own ticks and trusted context. This explicitly
scopes the phrase "one invocation per tick" to the existing clock domain.

If any scoped demand samples a collection member, suppress the collection's
unscoped schedule as a group, preventing a device read on a second unscoped
schedule. Other collections retain their schedules. Resume the unscoped group
from a fresh origin when its scoped demand ends, following the current rule.

### Proposed Wire Shape

Extend current stream declarations and Samples frames; the following shows only
the new fields and messages. Field numbers must be allocated against the combined
protocol tree, including the concurrent state worker's changes.

```proto
// New StreamDeclaration field: empty for an ordinary singleton observer.
string collection = 7;

// A new InstanceSamplesRequest frame, sent on a demand transition.
// Streams are the currently listened members within this Samples scope.
message CollectionDemand {
  repeated string streams = 1;
}

// A new InstanceSamplesResponse frame, acknowledging a demand transition.
// The host waits for first-start or last-stop cleanup before proceeding.
message CollectionDemandResponse {}

// A new InstanceSamplesResponse frame, answering part of a collection window.
message CollectionBatch {
  // Tick index within the outstanding Window, starting at zero.
  uint64 offset = 1;
  repeated CollectionRows streams = 2;
  // Finishes this tick, including one with no rows in any stream.
  bool end_tick = 3;
  // Completes the granted window; requires end_tick on its final tick.
  bool last = 4;
}

// Rows for one declared member, in publication order.
message CollectionRows {
  string stream = 1;
  repeated bytes rows = 2;
}
```

The existing `Window` carries start, period, count and sequence. Each output row's
timestamp is `start + offset * period`, stamped by the host. All rows from a tick
therefore share exactly one timestamp. Missing stream entries or empty row lists
mean silence; they do not produce rows, sequence gaps or failure markers. An empty
completed tick must still be acknowledged so scoped waits can settle.

Parts remain ordered; split large results at row boundaries using the current
transport message ceiling. Multiple parts may share an offset until `end_tick`;
the next part then advances the offset by one. Reject undeclared stream IDs,
duplicate groups within a part, skipped/completed tick offsets, invalid terminal
flags and any traffic outside an active window. A single row exceeding the
transport ceiling fails with an actionable resource-exhausted status.

Keep dense `SampleBatch` columns for `Observer::window`, which has real bulk
semantics. Share demand, scheduling, context, validation and lifecycle ownership;
encoding dense columns and sparse rows are separate representations, not separate
schedulers. `Observer::new` is a one-member handler adapter in that machinery.

### Output, Failure and Capacity

`publish` accepts only a registered member with its declared message type. It
encodes and preserves each stream's row order. Publishing for an unlistened member
is permitted; the host stores only the members in that tick's demand snapshot.

A handler error discards the unanswered window's staged output and fails all
listened members of that collection with `WindowFailed`. It leaves unrelated
collections active, using the same attachment/scoped failure reporting as current
observers. Do not turn errors into empty ticks or emit partial success before the
handler succeeds. Capacity failures retain the existing fail-fast behavior.

Variable row counts invalidate the current `Row::least(count)` preflight as a
sufficient capacity proof. Implementation must account actual staged bytes and
rows against the existing publication pool, releasing claims on every failure.
Do not introduce a second output budget, unbounded response staging, a new queue
or a product-side staging service. Reuse existing publication admission and
accepted-ingestion ownership; if that cannot stage a successful window safely,
report the missing capability before adding a competing mechanism.

### First and Last Listener Hooks

The host sends an explicit collection-demand transition when the listened member
set changes. Idle eagerly opened Samples calls have an empty set and acquire no
resources. Count active demands across the collection's scopes within the SDK's
existing served plugin instance; do not count individual rows or member streams.

On zero to one active demand, await `on_start` before granting the first tick.
Further members or overlapping scopes do not call it again. On one to zero,
cancel and settle the final in-flight handler, then await `on_stop` before
acknowledging stop. A start failure acquires no active demand and fails its caller;
partially acquired resources must be released by that failing start operation.
A stop failure reports its cause and still removes the demand's ownership.

Reuse attachment settlement, cancellation and `stop_grace`; no new delay, timeout
or retry. Half-close, cancellation, handler failure and transport loss remove
their demand share exactly once. Attachment settlement waits for cleanup within
its current deadline; local RAII release must cover cancellation of an async stop
hook. A remote plugin cannot be guaranteed to execute an async cleanup hook after
a process crash or permanent disconnection; the host guarantees ownership removal
and its existing bounded settlement, while process-local resources must also have
ordinary drop/close ownership.

Hooks use the first/last transitioning demand's Context. Their resource lifetime
spans all active scopes; per-scope data continues to belong to the scoped Context.
Hooks and collection handler execution must be coordinated so last-stop never
releases a resource still in use.

### Proposed Rust Authoring Shape

```rust,ignore
let collection = Collection::operation(async |ctx, out| {
    let reading = read_device(ctx).await?;
    out.publish(&STATE, reading.state);
    if let Some(link) = reading.link {
        out.publish(&LINK, link);
    }
    for item in reading.scan {
        out.publish(&SCAN, item);
    }
    Ok(())
})
.stream(STATE)
.stream(LINK)
.stream(SCAN)
.interval_setting("x.interval")
.on_start(open_device)
.on_stop(close_device);

ctx.register(collection)?;
```

Use an owned typed output handle so an async handler can publish without borrowing
a temporary buffer. `publish` latches encoding/membership/capacity errors; handler
completion checks that error before sending success, since the proposed surface
does not require `?` at every publication. `.every(Duration { .. })` supplies a
constant, matching observers. Registration installs its member definitions;
separate registration of the same definitions is refused. Typed stream handles
must remain usable as publication selectors after declaration ownership moves.

Retain existing ordinary observer authoring through adapters with one internal
owner. Delete superseded internal fields and dispatch once callers migrate; do
not retain aliases, fallback paths or a second collection registry.

Non-Rust SDK collection authoring is conditional on a thin adapter being possible.
Their existing observer transports must still accept the generated additive wire
contract. Do not implement an independent scheduler or lifecycle manager to claim
authoring parity; bindings alone are an explicit partial SDK outcome.

## Implementation Inventory After Activation

The orchestrator must establish the checkbox inventory in `tasks.md` after
activating this change; none of the following is complete:

1. Admit grouping and intervals; extend the wire and its behavior contract;
   regenerate every tracked binding and digest affected by the contract.
2. Generalize runtime demand preparation, gates, window response validation,
   ingestion and scoped suppression into collection ownership.
3. Generalize Rust registration/server dispatch and observer adapters; implement
   typed output and first/last listener hooks, deleting replaced internal paths.
4. Add deterministic runtime-double and SDK-harness proof for grouped invocation,
   zero/many rows, exact timestamps, errors, overlapping listeners, independent
   scopes, demand transitions, hook failures and bounded teardown.
5. Inspect non-Rust SDK adapters, implement only thin authoring, and record any
   missing capability. Regenerate combined outputs with the state worker's change.
6. Run format, touched-package test-target compilation, workspace test-target
   compilation when public signatures require it, focused tests, tooling tests,
   generation drift, repository checks and strict OpenSpec validation once on the
   finished candidate. Use native Windows proof here and report Linux as pending.

## Overlap Review

`sdk/rust/src/stream.rs:147` (`Observe`) and `:168` (`ObserveWindow`) both answer
observer windows, through `sdk/rust/src/server.rs:434`. The former computes rows
one at a time; the latter is used by bulk-column handlers, including
`sdk/rust/src/bin/invoke-fixture.rs:370`. Their computation granularity is required
behavior, and they already share server dispatch and host scheduling. They are
not competing schedulers. Retain both authoring adapters and extend their shared
owner for collections; do not introduce another dispatch/lifecycle owner.

The reported product-owned sleep-loop producer and host scheduling in
`crates/harmos-runtime/src/plugin/grpc/publish.rs:567` would both schedule device
reads. The host is called by attached/scoped publications; product call sites are
unavailable here. Recommend deleting the product loop after collection adoption,
but the owner must identify its files and approve that removal in its repository.

## Documentation Impact

No documentation files are edited. After implementation, the owner should update:

| File and Line | Required Statement |
| --- | --- |
| `docs/architecture.md:1258` | Samples demand groups requested member streams by collection within each scope. |
| `docs/architecture.md:1323` | Window answers include dense observer columns and collection rows with zero or more rows per tick. |
| `docs/architecture.md:1342` | Collection demand and unscoped suppression operate on a whole collection; row restrictions depend on the encoding. |
| `docs/architecture.md:1362` | A collection handler failure ends the collection's listened demands together. |
| `docs/python-sdk.md:64` | Document collection bindings and report whether Python authoring is available. |
| `docs/csharp-sdk.md:37` | Document collection bindings and report whether C# authoring is available. |
| `docs/cpp-sdk.md:138` | Document collection bindings and report whether C++ authoring is available. |

README impact: its gRPC placement feature at `README.md:51` remains true; a
collection example belongs beside the maintained SDK guidance after landing.

## Adoption

A product registers its existing stream definitions through one Collection,
returns each device read's rows through `out.publish`, and moves acquire/release
work into `on_start`/`on_stop`. Listener code keeps selecting stream IDs. The host
owns intervals, windows and scope clocks. Once verified in that repository,
delete the product's periodic producer task, sleeps, tick timer, listener counting
and cancellation glue whose sole purpose was scheduling that same device read.
Preserve independent device event producers and unrelated lifecycle behavior.
