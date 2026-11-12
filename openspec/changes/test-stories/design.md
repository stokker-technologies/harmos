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
| Transient retries run three attempts | 3 | `R/tests/operations/execution.rs:72`, `policy.rs` transient success, `outcomes.rs:265`; retain distinct policy and child settlement facets in the story |
| Authored request completes, cancels, panics | 3 | `R/tests/operations/wire/authored.rs:137,148,160`; keep distinct listed-declaration cases |
| Commit, undo, redo and availability | 4 | `R/tests/state/undo.rs`: undo/redo append, availability, read-only queries and forward-commit invalidation; preserve conflict and principal scenarios |
| Stored waveform is resumed, summarized and downsampled | 5 | `S/tests/integration/manager/nominal.rs:194,223,237,263,295`; retain file and native read stories separately |
| Mutation receipts prevent duplicate effects | 3 | `R/tests/storage/manager/production.rs:61,121,165`; replay, lost response and cancelled append; retain exclusive writes and integrity cases |
| Generated binding consumers | 8, generating two packages each | `P/tests/acceptance/bindings.rs:450..511`; moved to tooling owner, generate once |

## Workloads That Move to Benchmarks

| Test | Workload | Keeps as a Test |
| --- | --- | --- |
| `S/tests/integration/manager/mapping.rs:237` | 40,000 and 1,000,000 rows with timing | The 4,099-row case at line 231, without timing |
| `S/tests/integration/state/seven_day_recovery_seeks_and_compacts_without_rescanning_prefixes.rs:67` | 10,080 journal entries | A history just large enough to cross a page and a compaction boundary |
| `S/tests/integration/exact_quantile_spill.rs:46` | 4,101 values by 101 percentiles | One small forced-spill case |

## Real Time

Replaced by a controlled clock or an awaited event:

| Location | Dependency |
| --- | --- |
| `R/tests/operations/wire/grpc_network.rs:821,894` | Real 1 s and 50 ms listener deadlines |
| `R/tests/operations/wire/dynamic.rs` (renamed runtime-defined declarations) | 1 ms retry backoff and real expiry |
| Removed sidecar tests; current `R/tests/plugin_grpc/deadline.rs` and clock stories use host clock | Operations reach real policy deadlines |
| `R/tests/services/recovery.rs:248` | 10 ms stop timeout |
| `R/tests/services/shutdown.rs:508` | A 60 s rest raced against shutdown |
| `S/tests/support/file_backend.rs:67,88` | Receipt expiry from the system clock |
| `S/tests/integration/manager/mapping.rs:215` | Elapsed time in a correctness helper |
| `R/tests/system/guest_contract/limits.rs:101`, `operations.rs:153` | Real guest deadline expiry |

Kept as guards only: the notification, directory, batching, observer-demand and process bounds
that stop a hung test (`R/src/guest/host/plugin/scope.rs`, `S/tests/integration/directory.rs:53,172`,
`S/tests/integration/manager/batching.rs:1070`, current `R/tests/plugin_grpc` demand stories,
`R/tests/system/schematic.rs:99`). Each waits for its event first.

## Tests Removed

| Test | Reason |
| --- | --- |
| `R/tests/operations/lanes.rs`: lane name/capacity readback tests | Repeat declared names and capacities; admission is tested at lines 130 and 136 |
| `R/tests/state/managed.rs:111` | Reads back the value it set through a clone without mutating either handle |
| `crates/harmos-operations/src/lib.rs`: fixed catalog-size assertion | Pins the catalog to 30 entries; uniqueness moves into a story |
| `crates/harmos-guest/tests/kit/main.rs:75` | Pins fixture names and literal timeouts; useful checks move into a guest story |

## Deferred Scope

Relaxing System test serialization and splitting the largest crates are deferred until the
removals have landed and can be measured. Keep current serial System execution while rewriting
stories. The host-clock measurement fixture now uses the captured host clock; the other listed
real-time dependencies still need individual inspection. JSON-backend-only workloads are removed
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
