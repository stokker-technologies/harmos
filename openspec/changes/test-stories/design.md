# Test Stories Design

`R` is `crates/harmos-runtime`, `S` is `crates/harmos-storage`, and `P` is `tools/packaging`.

## Developer Feedback Scope

Focused selection builds only selected Cargo targets and their dependency closure. A story
uses its module prefix; an exact case uses its full test name. Changed-path selection must
include dependent packages for product changes and every target that shares changed test
support. Unknown paths fail with an actionable diagnostic rather than imply successful proof.
Existing native hang guards apply to focused runs too. The Tests archive remains the hosted
producer and is never a prerequisite of a local story.

The conformance manifest disables empty executable harnesses, retaining all fixture binaries
and their behavior stories. No test function or conformance case is removed by that selection.

The retained storage inventory additionally includes only the inline test modules in
`crates/harmos-storage/src/query/{execute,accumulate,spill,table}.rs`, the opened-offset
story in `tests/integration/manager/files.rs`, and new spill/opened-file workloads in
`benches/query/`. No production function, page limit or memory budget changes. The merge
story retains three real output pages while supplying three source pages, rather than
creating hundreds of tiny temporary runs. Quantile tests retain exact integer clone
branches, signed/duplicate float ranks and fractional interpolation with forced spill;
the week-sized floats move to benchmarks. Spill ownership still crosses the actual page
limit and verifies cleanup. Table ownership still crosses producer pages and survives
its owner; long histories live in benchmarks.

## Shape of a Story

```rust
#[tokio::test(start_paused = true)]
async fn undo_and_redo_follow_commits() -> Result<(), TestError> {
    // Start
    let mut app = App::launch(Bank::catalog()).await?;

    // Commit, then undo
    app.commit(Deposit { amount: 19 }).await?;
    assert!(app.can_undo());
    app.undo().await?;
    assert_eq!(app.balance(), 0);

    // Redo is available until a new commit replaces it
    assert!(app.can_redo());
    app.commit(Deposit { amount: 5 }).await?;
    assert!(!app.can_redo());

    // Stop
    app.shutdown().await
}
```

## Shared Modules

| Module | Replaces |
| --- | --- |
| Runtime launch, catalog registration and shutdown | Existing `R/tests/support/app.rs` Bank launch helpers plus remaining inline generic launches; extend shared launch, registration and shutdown mechanics |
| One storage fixture | The two fixture shells in `R/tests/support/storage_fixture.rs` and `S/tests/integration/manager/fixture.rs`, which share 60 lines |
| Wait for an event | One-poll and yield-loop helpers in `R/tests/support/pending.rs`, the storage fixtures, |
| Providers and hosts | Repeated fake providers in `R/tests/plugin_grpc/` (already rewritten with `grpc-plugins`) and `R/tests/operations/` |

## Workloads That Move to Benchmarks

| Test | Workload | Keeps as a Test |
| --- | --- | --- |
| `S/tests/integration/manager/mapping.rs` and `S/benches/query/projection.rs` | 40,000 and 1,000,000 rows with timing | The 4,099-row correctness story, without timing |
| `S/tests/integration/state/recovery_seeks_and_compacts_across_page_boundaries.rs` | 10,080 journal entries | 32 entries crossing a 31-entry page; a 10-entry tail survives compaction and crash restoration |
| `S/tests/integration/exact_quantile_spill.rs:46` | 4,101 values by 101 percentiles | Small signed/extreme-value parity; 1,025 inputs cross the cancellation-check boundary |

## Time and Hang Guards

Kept as guards only: the notification, directory, batching, observer-demand and process bounds
that stop a hung test (`R/src/guest/host/plugin/scope.rs`, `S/tests/integration/directory.rs:53,172`,
`S/tests/integration/manager/batching.rs:1070`, current `R/tests/plugin_grpc` demand stories,
`R/tests/system/schematic.rs:99`). Each waits for its event first.

## Deferred Scope

Relaxing System test serialization and splitting the largest crates are deferred until the
removals have landed and can be measured. Keep current serial System execution while rewriting
stories. JSON-backend-only workloads are removed
by `facility-trim`, not recreated as benchmarks or counted as completed story consolidation.

## Current Crate Inventory

| Owner | Scope |
| --- | --- |
| Runtime | `tests/support`, `tests/operations`, `tests/state`, `tests/storage/manager`, listed service and guest deadline tests; shared launch mechanics adopted by runtime consumers |
| Storage | `tests/integration/manager`, `tests/integration/state`, quantile spill, `tests/support/file_backend.rs`, `benches/query` |
| Operations | Inline catalog unit test removed; uniqueness and fingerprints are checked in the runtime built-in control story |
| Guest | `tests/kit/main.rs`: remove fixture-name/timeout readback; retain behavior in system guest stories |
| Protocol, Domain, Rust SDK/derive, examples | Inspected for relocation effects; existing independent journeys retained; no obsolete macro or sidecar tests recreated |
| Tools/packaging | Binding consumers under `tests/acceptance/bindings/`; one export serves all consumer facets |

SDK client tests live in `R/tests/operations/wire/sdk_client.rs` and `sdk/rust/tests/integration`.
Declarations and guest machinery moved to operations; conformance lives in `tools/conformance`.
The binding consumer story is maintained with the crate stories in this checkout.

## Test Mechanics Decisions

Shared storage construction accepts the native storage manager factory, so runtime keeps its
resident-owned worker while storage tests retain their direct worker owner. Teardown assertions
are visible in stories. Backend stages use `events::until` with fixture notifications; fixed
scheduler-turn loops and duplicated pending helpers are removed. Immediate pending observations
at in-process admission boundaries remain visible checks while the fixture holds the exact
resource being contested; they do not infer progress from time or scheduler yields.
Mutation-window expiry uses this same source, with no live monotonic-time offset.
The manual test clock is a test-only `Clock` implementation using watch notifications. It can
advance deadlines while a deliberately stalled handler remains active, whereas the production
virtual clock correctly refuses to advance during active external work. Clock accounting itself
continues to use the existing virtual-clock stories.

## Coverage Decisions

- Recovery faults remain separate: each fault interrupts a different persistence or cleanup owner.
- gRPC network/endpoint cases remain distinct where native and transcoded envelopes have different authority or settlement paths.
- Opened/frozen file cases remain distinct where source mutation, cancellation, unlinking or Windows handle access change ownership.

The canonical Tests producer and its Rust lint selector explicitly enable
`harmos-runtime/plugin-guest` and `harmos-packaging/generate-python`. Moving packaging
out of runtime removed incidental feature unification; the archived proof must retain both
the guest host System selection and generated binding consumers. Shipping features are unchanged.

The System task accepts the existing nextest filter shape while retaining serial execution
and its configured hang guards. Focused guest proof uses this selection from the shared archive.
