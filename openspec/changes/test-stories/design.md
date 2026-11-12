# Test Stories Design

Status: inventory locked against the current combined tree on 2026-11-12. `R` is
`crates/harmos-runtime`, `S` is `crates/harmos-storage`, `P` is `tools/packaging`.
The combined `grpc-plugins` marker authorizes implementation. Removed facilities are excluded.
The current worker owns this checkout, including tooling and packaging paths. The owner has
authorized binding consolidation (3.4) here; there is no concurrent writer in this worktree.
The developer selections are documented in `docs/tooling.md`; product README claims remain unchanged.

## Developer Feedback Scope

The speed-tests worktree owns test selection in `tools/python/src/harmos_tools/ci/tasks.py`,
new focused-test tooling and its tests, retained Rust test/benchmark stories and their
mechanical support, `docs/tooling.md` and this change's task/design records. The runtime
manifest's benchmark fixture selector and mise's matching benchmark/lint feature selections
also belong here: ordinary integration builds must not build the benchmark server. Other build producer
and Cargo configuration changes remain with the separate build worker. No product source
test seam is planned. Guest lifecycle/adapter suites under grpc-plugins 7.1 are measured
but retain their current implementation and System serialization.

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

## Groups That Become One Story

| Scenario | Tests Today | Location |
| --- | --- | --- |
| Transient retries run three attempts | 3 | `R/tests/operations/retries.rs`; identity, fixed backoff, parent/child attempt settlement share one story |
| Authored request completes, cancels, panics | 3 | `R/tests/operations/wire/authored.rs`; distinct declaration facets remain in `authored_listed.rs` |
| Commit, undo, redo and availability | 4 | `R/tests/state/undo_redo.rs`; conflict, barrier and principal scenarios remain in `undo.rs` |
| Stored waveform is resumed, summarized and downsampled | 5 | `S/tests/integration/manager/nominal.rs`; `file_ranges.rs` and `native_stream_reads.rs` are separate journeys; shared mechanics in `tests/support/nominal.rs` |
| Mutation receipts prevent duplicate effects | 3 | `R/tests/storage/manager/receipts.rs`; exclusive writes and integrity cases remain in `production.rs` |
| Generated binding consumers | 9 consumer facets, generating two packages each | `P/tests/acceptance/bindings/`; one export shared by every consumer facet |

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
| `R/tests/system/guest_contract/limits.rs::manager_bounds_wait`, `operations.rs::shutdown_retains_cleanup`, `dependency_admission.rs` | Paused Tokio clock; native proof uses rebuilt guest fixtures |

Kept as guards only: the notification, directory, batching, observer-demand and process bounds
that stop a hung test (`R/src/guest/host/plugin/scope.rs`, `S/tests/integration/directory.rs:53,172`,
`S/tests/integration/manager/batching.rs:1070`, current `R/tests/plugin_grpc` demand stories,
`R/tests/system/schematic.rs:99`). Each waits for its event first.

## Tests Removed

| Test | Reason |
| --- | --- |
| Lane name/capacity readbacks from the former `R/tests/operations/lanes.rs` | Repeat declared values; `R/tests/operations/lane_validation_and_capacity_control_admission.rs` proves admission |
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

No product test seam was added. The operations library edit removes an inline test only; runtime and storage clocks/events are test-support code.

## Guest Initialization Regression

The foundation reduction (`e4eb6cf3`) moved inventory decoding and validation from
`harmos-domain` to `harmos-operations`. Both guest workspaces retained only the former
package's development optimization override, so the moved code consumed unoptimized Wasm
instructions within the unchanged fuel allowance. Row metadata decoding already existed at
`8f176ef1`; the inventory move lost its compilation policy. Both manifests now optimize
`harmos-operations` at the existing level. Native guest proof uses freshly built components;
fuel, deadlines, memory limits and System serialization stay unchanged.

## Second Reduction Pass

The source inventory includes runtime lane admission, principal undo availability, clock,
mutation receipts and snapshots; storage file readers, mapping and object codecs; their
shared fixtures and existing runtime/storage benchmark targets. Binding consumer facets
share one export. Lane capacities and snapshot payload boundaries are tables; principal
availability is checked before the same undo. Identical snapshot launches reuse the existing
fixture. The clock source-spelling scan and identical handwritten catalog readbacks are
removed while behavioral clock/codec stories remain. Empty skeleton headings are removed
throughout both test trees; this mechanical cleanup is recorded separately from behavioral
consolidation. The application catalog story no longer tests its own handwritten conversions
or decodes the same inventory twice through aliases.
The edited lane, principal, snapshot, reader, mapping and codec stories have individual files;
their shared modules retain mechanical fixture definitions.

The 2,048-file receipt workload and 730-position large-state snapshot workload live in the
existing benchmark targets. Correctness retains receipt admission across its eviction bound
and a 12-position segmented snapshot with an independent journal replay, keyed receipts,
undo, redo and a post-checkpoint commit. File round trips keep a small payload and a payload
crossing the 31-byte batch boundary. File retry, completion, slice and projection stages
share one file; mapped queries share one snapshot.

- Recovery faults remain separate: each fault interrupts a different persistence or cleanup owner.
- gRPC network/endpoint cases remain distinct where native and transcoded envelopes have different authority or settlement paths.
- Opened/frozen file cases remain distinct where source mutation, cancellation, unlinking or Windows handle access change ownership.

The canonical Tests producer and its Rust lint selector explicitly enable
`harmos-runtime/plugin-guest` and `harmos-packaging/generate-python`. Moving packaging
out of runtime removed incidental feature unification; the archived proof must retain both
the guest host System selection and generated binding consumers. Shipping features are unchanged.
The pending-shutdown story matches the retained typed timeout instead of a Debug substring
that stopped matching when `TimedOut` gained structured policy fields.

The System task accepts the existing nextest filter shape while retaining serial execution
and its configured hang guards. Focused guest proof uses this selection from the shared archive.

Guest System proof covers dependency admission and its retained deadline with paused Tokio
time, including successful startup under the unchanged provider budget. Guest child routing
selects the locally authored request declaration before validating
the assigned wire fingerprint, so distinct request types with identical fields cannot alias
one another. The declarations already belong to the plugin; no public caller shape changes.
Encoded guest-host probes use the authoritative protobuf host boundary, and preflight/retirement
checks compare the surrounding host inventory so built-in clock operations remain present.
