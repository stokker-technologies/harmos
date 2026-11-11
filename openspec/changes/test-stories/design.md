# Test Stories Design

Status: consolidation remains unimplemented. Recheck locations against the combined branch;
removed facilities are outside the story inventory. `R` is `crates/harmos-runtime`, `S` is
`crates/harmos-storage`, `P` is `crates/harmos-packaging`.

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
| Runtime launch, catalog registration and shutdown | 219 launches, 100 registrations and 667 shutdowns written inline across `R/tests` |
| One storage fixture | The two fixture shells in `R/tests/support/storage_fixture.rs` and `S/tests/integration/manager/fixture.rs`, which share 60 lines |
| Wait for an event | One-poll and yield-loop helpers in `R/tests/support/pending.rs`, the storage fixtures, |
| Providers and hosts | Repeated fake providers in `R/tests/sidecar/` (rewritten with `grpc-plugins`) and `R/tests/operations/` |

## Groups That Become One Story

| Scenario | Tests Today | Location |
| --- | --- | --- |
| Transient retries run three attempts | 3 | `R/tests/operations/execution.rs:74`, `policy.rs:381`, `outcomes.rs:217` |
| Authored request completes, cancels, panics | 3 | `R/tests/operations/wire/authored.rs:114,125,137` |
| Commit, undo, redo and availability | 4 | `R/tests/state/undo.rs:63,101,164,178` |
| Stored waveform is resumed, summarized and downsampled | 5 | `S/tests/integration/manager/nominal.rs:203,232,246,272,304` |
| Mutation receipts prevent duplicate effects | 3 | `R/tests/storage/manager/production.rs:60,111,160` |
| Generated binding consumers | 8, generating two packages each | `P/tests/acceptance/bindings.rs:438..537`; generate once |

## Workloads That Move to Benchmarks

| Test | Workload | Keeps as a Test |
| --- | --- | --- |
| `S/tests/integration/manager/mapping.rs:242` | 40,000 and 1,000,000 rows with timing | The 4,099-row case at line 236, without timing |
| `S/tests/integration/state/seven_day_recovery_seeks_and_compacts_without_rescanning_prefixes.rs:67` | 10,080 journal entries | A history just large enough to cross a page and a compaction boundary |
| `S/tests/integration/exact_quantile_spill.rs:46` | 4,101 values by 101 percentiles | One small forced-spill case |

## Real Time

Replaced by a controlled clock or an awaited event:

| Location | Dependency |
| --- | --- |
| `R/tests/operations/wire/grpc_network.rs:821,894` | Real 1 s and 50 ms listener deadlines |
| `R/tests/operations/wire/runtime_defined.rs:154` | 1 ms retry backoff and real expiry |
| `R/tests/sidecar/dependency_routing.rs:363,437`, `listeners.rs:227` | Operations reach real policy deadlines |
| `R/tests/services/recovery.rs:248` | 10 ms stop timeout |
| `R/tests/services/shutdown.rs:508` | A 60 s rest raced against shutdown |
| `S/tests/support/file_backend.rs:67,88` | Receipt expiry from the system clock |
| `S/tests/integration/manager/mapping.rs:215` | Elapsed time in a correctness helper |
| `R/tests/system/guest_contract/limits.rs:101`, `operations.rs:153` | Real guest deadline expiry |

Kept as guards only: the notification, directory, batching, observer-demand and process bounds
that stop a hung test (`R/src/plugin/scope.rs:791`, `S/tests/integration/directory.rs:53,172`,
`S/tests/integration/manager/batching.rs:1070`, `R/tests/sidecar/observer_demand.rs:632`,
`R/tests/system/schematic.rs:99`). Each waits for its event first.

## Tests Removed

| Test | Reason |
| --- | --- |
| `R/tests/operations/lanes.rs:223,231` | Repeat declared names and capacities; admission is tested at lines 130 and 136 |
| `R/tests/state/managed.rs:111` | Reads back the value it set through a clone without mutating either handle |
| `crates/harmos-operations/src/lib.rs:197` | Pins the catalog to 30 entries; uniqueness moves into a story |
| `crates/harmos-guest/tests/kit/main.rs:75` | Pins fixture names and literal timeouts; useful checks move into a guest story |

## Deferred Scope

Relaxing System test serialization and splitting the largest crates are deferred until the
removals have landed and can be measured. Keep current serial System execution while rewriting
stories. The host-clock measurement fixture now uses the captured host clock; the other listed
real-time dependencies still need individual inspection. JSON-backend-only workloads are removed
by `facility-trim`, not recreated as benchmarks or counted as completed story consolidation.
