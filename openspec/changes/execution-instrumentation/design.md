# Execution Instrumentation Design

## Agreed Direction

Existing `tracing` owns spans and events. No profiling crate, feature, correlation-ID generator or global subscriber installation belongs in the production library. DEBUG spans describe requests and lifecycle; TRACE spans describe batches/pages. Consumer applications choose runtime filters, collector budgets and release filtering. Do not statically disable levels in Harmos.

## Source Inventory And Ownership

| Owner | Files And Responsibilities |
| --- | --- |
| Runtime Worker | `harmos-runtime/src/operations/**`, `supervision/{executor,instance}.rs`, `state/**`, `storage/**`, `grpc/**`, `guest/host/{serving,session}.rs`, `services/sidecar/{mod,connection,exchange}.rs`, `runtime.rs`; operation admission, execution/residents, queues, physical settlement, state capture/readers and transport invocation context. `harmos-sdk/src/operations/**` and stream endpoints, `harmos-sidecar/src/**`, `harmos-guest/src/kit/**` where shared execution paths require propagation. Associated integration tests. |
| Storage Worker | `harmos-storage/src/**` and integration tests; manager admissions/status/lifecycle, every backend call boundary, worker batching, queries/readers, snapshots, files/staging/deletion, recovery and blocking I/O propagation. |
| Report Worker | Test-only capture helper and system-story wiring in runtime/guest tests; exporter qualification and fixture capture lifecycle. `examples/schematic/{cli,sidecar}/src/main.rs` application-owned optional capture; `examples/schematic/app/src/services/sidecar.rs` explicitly whitelists the report directory for that fixture. Criterion benchmark configuration consumers. Investigate upstream Hub report replacement; no concurrent edits to the active Hub owner's files. |
| Orchestrator | All Cargo manifests/locks, mise task inputs, OpenSpec, README/CONTRIBUTING and integration review/proof. Canonical Hub regeneration owns affected `tools/python/**`, `.github/**`, `.gitlab/**`, `crates/cargo-harmos/templates/{guest,sidecar}/**`, repository checks/configuration and managed guidance copies needed to keep native reports, artifact retention and generated checks consistent. Resolve upstream generated-owner changes before rendering; never hand-edit generated copies or synchronize provider protections during migration. Hub source belongs exclusively to its existing owner; this task may run the user-authorized canonical reinstall but must not edit or commit Hub source. |

All operation variants flow through central dispatch; coverage includes residents, streamed endpoints, guest/native/sidecar and gRPC. All storage interfaces includes admission, writes, reads and subsequent pages, queries, snapshots, file production/following/deletion, retention, recovery, receipts, shutdown and backend invocation. Pure value accessors and per-element mathematical kernels do not need duplicate spans.

## Correlation And Lifetimes

Create context before enqueue. Work envelopes retain span and subscriber where scoped collectors need it. Async futures use `Instrument`; never hold entered guards across await. Close queue-residence spans on dequeue and keep physical execution spans until settlement, including cancellation and failure.

Scoped subscriber ownership includes destruction, not just polling: the registry closes ancestor references through the current dispatcher. Private owner-local wrappers restore the saved dispatcher for both poll and Drop at handoffs; reuse existing boxed futures where possible. Additional boxing is confined to cross-context work, carries a small allocation cost and is not claimed to be free. Captured span handles and queue spans are also dropped under their owning dispatcher. Cancellation tests exercise this boundary.

Reuse operation IDs/generations, request keys, journal positions and storage cursors as structured fields. Never record request/response payloads, arbitrary principal data, credentials or paths with sensitive content. Internal handles stay out of domain models and persistence.

Separate admission waits, queue waits, locks, preparation/validation/apply, publication, read/scan/accounting/conversion/delivery, backend writes and durability waits. Each request's queue links to a contribution span using `follows_from` within its owning subscriber. Contributions share existing batch identity fields with the single physical-work span; receipt/frontier fields connect waiters to shared persistence. The user approved this representation because tracing provides no public subscriber-identity comparison and numeric span IDs can collide across collectors. Never infer collector compatibility by probing current-span behavior or pass foreign span IDs into another subscriber. Do not sum overlapping contribution/stage durations or claim span duration is CPU time.

Cross-process reports expose existing invocation identities, provider/process identity and generation at send and receive boundaries for structured joins. Local span IDs are never treated as portable. Prepare/initialize/finalize lack portable values in the current carrier protocol; do not change that protocol or invent a reserved trace value. These phases correlate by invocation identity, not automatically drawn cross-process span flows. Local parent/causal links remain explicit. Any future wire propagation is a separate agreed contract.

## Native Artifacts

Qualify dev-only `tracing-perfetto-file` with async span tracks and causal-flow support. Use an existing exporter, no custom profiler. Test-owned guards cover runtime settlement and explicitly flush on normal completion; unwind preserves available evidence. Artifact names distinguish process/story instances. Partial traces from killed processes cannot claim completion.

The native exporter records process command-line and source-location metadata. The fixture CLI accepts plugin paths and a mode flag, not credentials or request payloads. Span payload exclusion does not imply artifact sanitization; document native path metadata rather than adding a custom serializer or exporter fork.

The exporter emits pending causal flows at span boundaries. Queue handoff spans precede contribution spans: link before retiring the queue span, and close it before dependent work finishes. Join shared physical work through the batch identity fields. Do not use a full awaiting request as a predecessor whose close follows the completed batch. Qualify emitted relationships against the native protobuf artifact, not merely subscriber callbacks.

Keep native Criterion raw results and HTML with Plotters so no external plotting executable is required. Remove the custom benchmark CSV/Markdown rendering and its consumers at the upstream generated owner, preserving baseline identity/comparison and regression findings. Native JUnit remains the test-verdict artifact, not a duplicate performance report.

Hub `machine-work-root` tasks 6.85/6.86 already own native report replacement and runner report-directory injection in `hub.wt/fixes`. The user transferred Hub fixes to its dedicated owner chat and authorized direct handoffs there; Harmos does not edit those concurrently owned files. The canonical generated runner replaces custom `bench.csv`/`bench.md` with native benchmark artifacts. The user explicitly selected Perfetto with causal links over Hub's earlier tracing-chrome choice. Hub's environment contract is `STKR_TEST_REPORT_DIR` under `.stkr/traces/<profile>/<native>/<existing-lane-key>`; Harmos writes distinct native files under its `traces/` child. Adopt the owner's validated native-file transport and generated templates, without a local duplicate runner. Criterion HTML is Harmos's choice, not a mandatory Hub-wide feature.

## Validation And Delivery

Only targeted deterministic checks during implementation: span coverage/fields, handoff ancestry, shared-batch links, cancellation/error closure, filtering, artifact flushing and exporter-readable relationships. No timing thresholds or authored-prose assertions. Add/use explicit project tasks with compiler-output preflight for focused compilation; no direct Cargo compile commands.

After every implementation item and handoff closes, review complete coverage. The user authorized reinstalling the latest Hub and integrating this complete feature into `dev`, including push, ready PR, required checks and native auto-merge. Full native validation remains pending until the required authorized delivery flow; no benchmarks run as incidental per-change proof. Resolve published dependency pins where coupled runtime/library changes apply. Hub's generator supplies committed, repository-owned Python/workflow sources; the public Harmos pipeline does not acquire a private Hub runtime dependency.
