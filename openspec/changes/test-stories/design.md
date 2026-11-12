# Test Stories Design

Status: inventory locked against the current combined tree on 2026-11-12. `R` is
`crates/harmos-runtime`, `S` is `crates/harmos-storage`, `P` is `tools/packaging`.
The combined `grpc-plugins` marker authorizes implementation. Removed facilities are excluded.
Tests, fixtures and benches in crates, the Rust SDK and examples belong to this worker;
`tools/**`, `.github/**`, `mise.toml` and remaining packaging paths belong to the concurrent worker.
Binding consolidation (3.4) therefore requires that owner’s handoff; this worker does not edit it.
No README claims or public usage change.

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

## Groups That Become One Story

| Scenario | Tests Today | Location |
| --- | --- | --- |
| Transient retries run three attempts | 3 | `R/tests/operations/retries.rs`; identity, fixed backoff, parent/child attempt settlement share one story |
| Authored request completes, cancels, panics | 3 | `R/tests/operations/wire/authored.rs`; distinct declaration facets remain in `authored_listed.rs` |
| Commit, undo, redo and availability | 4 | `R/tests/state/undo_redo.rs`; conflict, barrier and principal scenarios remain in `undo.rs` |
| Stored waveform is resumed, summarized and downsampled | 5 | `S/tests/integration/manager/nominal.rs`; `file_ranges.rs` and `native_stream_reads.rs` are separate journeys; shared mechanics in `tests/support/nominal.rs` |
| Mutation receipts prevent duplicate effects | 3 | `R/tests/storage/manager/receipts.rs`; exclusive writes and integrity cases remain in `production.rs` |
| Generated binding consumers | 8, generating two packages each | `P/tests/acceptance/bindings.rs:450..511`; moved to tooling owner, generate once |

## Workloads That Move to Benchmarks

| Test | Workload | Keeps as a Test |
| --- | --- | --- |
| `S/tests/integration/manager/mapping.rs` and `S/benches/query/projection.rs` | 40,000 and 1,000,000 rows with timing | The 4,099-row correctness story, without timing |
| `S/tests/integration/state/recovery_seeks_and_compacts_across_page_boundaries.rs` | 10,080 journal entries | 32 entries crossing a 31-entry page; a 10-entry tail survives compaction and crash restoration |
| `S/tests/integration/exact_quantile_spill.rs:46` | 4,101 values by 101 percentiles | Small signed/extreme-value parity; 1,025 inputs cross the cancellation-check boundary |

## Real Time

Replaced by a controlled clock or an awaited event:

| Location | Dependency |
| --- | --- |
| `R/tests/operations/wire/grpc_network.rs:821,894` | Real 1 s and 50 ms listener deadlines |
| `R/tests/operations/wire/dynamic.rs` (renamed runtime-defined declarations) | The old retry/expiry story is gone; current dynamic tests exercise codecs synchronously |
| Removed sidecar tests; current `R/tests/plugin_grpc/deadline.rs` and clock stories use host clock | Operations reach real policy deadlines |
| `R/tests/services/recovery.rs::timeout_retains_cleanup` | Controlled clock waits for timer registration before advancing the unchanged 10 ms stop budget |
| `R/tests/services/shutdown.rs::domains_outlives_clones` | Awaits `context.stopping()` |
| `S/tests/support/file_backend.rs` | Fixture-owned date; restart story checks retention before expiry and removal at expiry |
| `S/tests/integration/manager/mapping.rs` | Timing is removed; large projections live under benches |
| `R/tests/system/guest_contract/limits.rs::manager_bounds_wait`, `operations.rs::shutdown_retains_cleanup` | Paused Tokio clock; native proof requires rebuilt guest fixtures |

Kept as guards only: the notification, directory, batching, observer-demand and process bounds
that stop a hung test (`R/src/guest/host/plugin/scope.rs`, `S/tests/integration/directory.rs:53,172`,
`S/tests/integration/manager/batching.rs:1070`, current `R/tests/plugin_grpc` demand stories,
`R/tests/system/schematic.rs:99`). Each waits for its event first.

## Tests Removed

| Test | Reason |
| --- | --- |
| `R/tests/operations/lanes.rs`: lane name/capacity readback tests | Repeat declared names and capacities; admission is tested at lines 130 and 136 |
| `R/tests/state/managed.rs:111` | Reads back the value it set through a clone without mutating either handle |
| `crates/harmos-operations/src/lib.rs` unit inventory test | Fixed count was already gone; uniqueness and nonzero fingerprints now belong to `R/tests/operations/host.rs::builtin_controls_instance` |
| `crates/harmos-guest/tests/kit/main.rs:75` | Pins fixture names and literal timeouts; useful checks move into a guest story |

## Deferred Scope

Relaxing System test serialization and splitting the largest crates are deferred until the
removals have landed and can be measured. Keep current serial System execution while rewriting
stories. The host-clock measurement fixture now uses the captured host clock; the remaining guest proof is tracked separately from native story proof. JSON-backend-only workloads are removed
by `facility-trim`, not recreated as benchmarks or counted as completed story consolidation.

## Current Crate Inventory

| Owner | Scope |
| --- | --- |
| Runtime | `tests/support`, `tests/operations`, `tests/state`, `tests/storage/manager`, listed service and guest deadline tests; shared launch mechanics adopted by runtime consumers |
| Storage | `tests/integration/manager`, `tests/integration/state`, quantile spill, `tests/support/file_backend.rs`, `benches/query` |
| Operations | Catalog unit test: retain uniqueness and construction behavior, remove fixed count |
| Guest | `tests/kit/main.rs`: remove fixture-name/timeout readback; retain behavior in system guest stories |
| Protocol, Domain, Rust SDK/derive, examples | Inspected for relocation effects; existing independent journeys retained; no obsolete macro or sidecar tests recreated |
| Tools/packaging | Binding consumers under `tests/acceptance/bindings.rs`; task 3.4 belongs to concurrent tooling owner |

SDK client tests live in `R/tests/operations/wire/sdk_client.rs` and `sdk/rust/tests/integration`.
Declarations and guest machinery moved to operations; conformance lives in `tools/conformance`.
No edits to those tool-owned paths are needed for crate story consolidation.

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

No product test seam was added. The operations library edit removes an inline test only; runtime and storage clocks/events are test-support code.

## Remaining Native Guest Proof

Rebuilt sandbox initialization consumes the existing fuel budget in
`harmos-operations/src/inventory/publications.rs::validate_match_with_settings` while decoding
stream metadata. Both guest deadline stories stop before their deadline assertions; task 4.2
remains open. Repair initialization with its runtime/operations owner, then rerun both exact
System stories. Raising fuel or time budgets and skipping the stories are outside this change.
