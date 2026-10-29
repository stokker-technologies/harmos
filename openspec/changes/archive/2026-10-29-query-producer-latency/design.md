# Query Producer Repair

## Ownership And Evidence

The native Harmos owner implements the producer slice; the orchestrator owns integration and final delivery. Dokime's existing raw, sorted and downsampled query APIs are the supported consumer boundary. Retained consumer evidence is under Dokime target/ci/targeted-1.52. Waiting for query output dominates decoding and statistics; this identifies the upstream measurement boundary, not the specific cause.

## Locked Contracts

Preserve full validation of stored payloads, schema and bounds; exact query values, ordering, null semantics and cursor behavior; cancellation and memory reservation lifetimes. Keep canonical unoptimized builds and consumer deadlines unchanged. Do not add a cache, bypass validation, increase concurrency or change response batching speculatively.

## Measurement And Repair

Measure aggregate storage scan, source conversion, plan execution, IPC construction, reservation waits and output delivery. Log durations and bounded counts only, never row payloads or credentials. Lock the smallest behavioral repair and regression inventory after these measurements identify the cause. Remove temporary diagnostics before source closure. Performance measurements are evidence, not timing assertions in correctness tests.

The temporary diagnostic inventory is limited to three existing source files:

- `crates/harmos-storage/src/query/reader.rs`: aggregate page admission, attempt/retry counts, backend scan, conversion and plan execution; separately measure batch reservation waits and IPC/envelope construction before delivery.
- `crates/harmos-storage/src/query/source.rs`: distinguish payload schema preparation, validation/accounting traversal, tree construction and Arrow array construction, with payload and row counts only.
- `crates/harmos-runtime/src/storage/operations/query.rs`: separate reader opening and advancement from output frame construction and send waits, preserving the existing grant checks, timeout and completion contract.

Dokime's retained rotation evidence delivered 413 rows through 21 queries and spent 13.37 seconds awaiting native output, versus about 37 milliseconds decoding IPC and 99 milliseconds encoding/decoding typed telemetry. Its four optimized plot reductions totaled about 5 milliseconds. One 100-row batch required about 3 seconds for each raw, sorted and downsampled read; this establishes the producer boundary without identifying scan, conversion, planning or transport as the cause. No explicit per-row delay was found in the inspected reader and runtime handler.

The regression candidate is the existing public storage integration boundary in `tests/integration/manager/mapping.rs` or `query_pressure.rs`, using the shared waveform schema and real worker lifecycle. Measurements should distinguish 1, 2 and 100 rows under raw, sorted and downsampled plans; correctness assertions must cover complete values, ordering, cursor completion and existing refusal behavior. A structural regression such as bounded backend scan or conversion attempts will be selected only if it represents the measured defect; elapsed-time assertions are not the correctness oracle.

## Measured Cause And Locked Repair

The locally patched diagnostic CLI retained in Dokime `target/ci/query-producer-1.2` reproduced rotation Stop at 17,494 milliseconds while ordinary Stop completed at 5,716 milliseconds. Compiler snapshots before and after observed no active compiler. For a 100-row sorted startup page, source conversion took 2,378,007 microseconds, including 2,371,596 microseconds in the validation/accounting traversal. Backend scan took 34,034 microseconds, tree construction 3,643 microseconds and Arrow construction 1,624 microseconds; admission took 2 microseconds with one attempt and no retry. The raw page showed the same dominant accounting traversal. Runtime output sends took only tens of microseconds.

Each accounting callback currently calls `MemoryScope::claim` for the event and every ancestor field. Reservation growth calls `Pool::charge`, which invokes `State::collect` twice; collection purges the complete retained receipt order. Repeating that global work for every local expansion increment explains the observed growth with retained history. Receipt size lookup itself is constant time. The repair does not change the ledger or capacity manager.

Production changes are limited to `crates/harmos-storage/src/query/source.rs`. A private accounting helper sums the same checked initial payload bytes times four, 256 bytes per event, each ancestor name length times four, and repeated length times 256. It makes one reservation claim for the counted page before allocating payload trees or Arrow arrays. Both existing payload traversals remain. On malformed input or arithmetic failure it first claims the successfully counted prefix, preserving reservation refusal precedence before propagating validation failure. Empty input adds no accounting claim.

Inline regression cases in that same file will assert the exact charge and one-claim structure for scalar, optional, repeated and nested fields; just-below and exact admission boundaries; malformed-prefix charging and refusal precedence; and checked-sum overflow. Existing public integration cases `manager::mapping::expansion_exceeds_budget`, `conversion_grows_reservation`, ordinary mapped/stream query cases and `manager::query_pressure::pressure_reloads_page` retain coverage of real reservations, refusal, recovery and allocation lifetime. No correctness test asserts elapsed time.

## Baseline Delivery Decision

Both native Stop journeys now pass with canonical unoptimized Products: ordinary RPC completes in 2.560 seconds and UI Saved in 2.925 seconds; rotation RPC completes in 3.723 seconds and UI Saved in 4.186 seconds. These are concurrent-load observations with different workloads, not an isolated benchmark. The reports contain 112 and 161 display samples, respectively. Retained source, artifact and phase evidence is in Dokime `target/ci/stop-full-path`.

The user chose validation and release of this baseline before further subsecond optimization. Finish commits take 25 and 32 milliseconds; report sealing dominates at 1.749 and 2.414 seconds. Report dependency file finish and flush waits justify a later storage-phase investigation, but no further performance changes are part of this delivery. All temporary query and state diagnostics are removed. The accounting repair and four deterministic regressions remain; README claims and public contracts are unchanged.

## Paired Development And Publication

The user cleared the paired Dokime target tree before its next compile. Earlier consumer raw probe artifacts are no longer present; minimal conclusions and the original registry lockfile are preserved under external stokker-recovery/dokime/hub-ci-clean-2026-10-25. Harmos's own targeted test outputs remain. All subsequent compilation, including focused diagnostics, must use project tasks with the standard cleanup preflight rather than direct Cargo plus a manual preflight.

The user's full Stop performance investigation now targets below one second, ideally below 500 milliseconds, without changing durability or compiler policy. Temporary commit attribution is limited to three Harmos runtime owners: state/manager.rs measures send admission and answer waits; state/writer.rs measures submission-to-dequeue, read/write/tail lock waits, check/prepare/inverse/apply, capture reservation/submission and publication; state/record.rs separates combined record collection/encoding from membership restoration. A private submission timestamp may travel in the internal commit request, without wire/API changes. Log existing definition identifiers, frontiers, counts and durations only. Preserve operation order and error behavior. Do not repeat serialization for diagnostics. Commit completion is not itself a disk flush barrier; filesystem internals remain out of scope until consumer durability measurements justify them. Dokime's paired task 1.54 owns report and transaction callback measurements. These measurements inform later optimization decisions rather than authorizing speculative changes.

Use an untracked checkout-local Cargo patch in Dokime and verify actual resolved package sources. Preserve its registry declarations; do not commit path resolution. After upstream source closure and required validation, publish through protected dev and the authorized release flow. Dokime then pins the verified published revision and removes the patch before final consumer validation. The user's source-complete push-first instruction applies, with serial local simulation alongside hosted CI and no CPU, memory or worker limits.

## Runtime Triage

The measured accounting repair landed in `b93d6f61`; later claim-free storage in `1b0630a2` supersedes that private accounting helper. Runtime diagnostic removal is already complete (task 1.5). Tasks 2.1–2.3 remain integration-owned; the measured sealing follow-up concerns file finish/flush, so no runtime optimization is inferred or storage code changed here.
