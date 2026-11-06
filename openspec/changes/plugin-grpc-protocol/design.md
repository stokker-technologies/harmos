# Plugin gRPC Prototype Design

## Scope and Order

This branch prototypes sidecars only on Windows, based on the landed unified Invocation over OperationHost. Guests remain Wasm; Postcard remains storage encoding. The existing framed carrier stays unchanged. Migration, harmos-protocol and SDK relocation remain deferred. Gates run in the owner's numbered order and a failed gate stops subsequent work.

## Baseline

Dokime's stream-ingest-performance design calls for bounded ordered batches, 4,096-row/1 MiB publication ceilings and a 2 ms partial flush policy. Its native fixture measures scalar/wide rows and 0/1/64 plots through durable Arrow storage, separating transport, writer and projection costs. Those storage measurements are not transport throughput. This prototype compares identical pre-encoded Arrow IPC batches on this Windows machine, reporting repetitions and spread. Encoding and verification stay outside timed transfer. The existing framed Connection and separate control channel provide the baseline.

## Source Ownership

harmos-sidecar owns the opt-in prototype module, Protobuf envelopes, authenticated generation admission and client. The Schematic sidecar owns the real ListDirectories process path and focused gate story. Existing production transport, guest, storage and SDK-layout sources are outside this inventory. README claims require review but the experimental feature is not advertised as a shipping carrier.

## Prototype Shape

`prototype-grpc` is opt-in in harmos-sidecar and the Schematic sidecar. The separate `schematic-grpc-prototype` executable listens on an ephemeral loopback address. Its parent supplies a random 256-bit token and generation on private stdin; stdout advertises only the bound address. Each RPC checks credentials, version and generation. Production entry points and manifests are unchanged.

The prototype Protobuf envelope is declared with Prost in `crates/harmos-sidecar/src/kit/prototype/codec.rs`. Control RPCs dispatch the existing WireTable without serializing its Postcard envelope. Ordinary request/response payloads use the SDK's existing Protobuf projection. Lifecycle requests are translated at the boundary into the existing internal representation; only Protobuf crosses the connection. Failure records cross as structured JSON bytes, retaining classification and diagnostic context. The handshake advertises plugin-specific lifecycle fingerprints.

The client implements OperationHost and Instance, so the real request uses `host.operation(ListDirectories {}).await`. Its deliberately narrow supported slice is unary operations without host callbacks, retry overrides, context values, process-local claims or typed operation endpoints. Unsupported options are refused explicitly. This is not a complete migration candidate or a conformance claim.

A separate streaming RPC repeats pre-encoded Arrow IPC batches for transport measurement, independently of control calls. Its producer reserves a bounded queue slot before copying a batch and selects cancellation while blocked. The experiment uses two queued batches, 64 KiB fixed HTTP/2 receive windows and a 2 MiB message ceiling. These are fixture-owned inputs, not library defaults. Arrow load is a transport fixture; the real operation remains the existing Schematic filesystem path.

## Validation Commands

All compilation and execution use `mise run build:rust:focused`; it runs the compiler-output preflight and matching Clippy. The process story is `examples/schematic/sidecar/tests/system/grpc_prototype.rs`, selected with `--manifest-path examples/schematic/Cargo.toml -p schematic-sidecar --features prototype-grpc --test system-prototype`. Its executable is resolved from the native runner's runtime binary environment variable. Local focused Cargo execution supplies that variable to the built worktree-local executable. Final source checks use `mise run check:rust`. No full simulation, PR or merge is part of this slice.

## Gate Results on Windows

### Gate 1: Fail — Real Sidecar Initialization

The first actual execution of `grpc_prototype::gate_1_real_sidecar` failed after 3.05 seconds (one failed test, zero passed). The focused mise task returned exit code 1. The child successfully bound its loopback endpoint, authenticated the parent, admitted the lifecycle fingerprint and dispatched the Protobuf initialization request through the existing SDK owner. Initialization then refused the Schematic operation table because its publication dependency had no host assignment:

```text
caller may not invoke operation `host.publish` without a host assignment
```

The structured failure survived the gRPC boundary as `Permanent`, with plugin origin and the complete lifecycle/table/binding diagnostic chain. The prototype supplies empty assignments and explicitly refuses host callbacks; the existing Schematic plugin requires its publication assignment even though ListDirectories itself does not publish. No successful end-to-end real operation was demonstrated. This is a failed first gate, not permission to narrow the plugin catalog, add a publication stub, grant broader authority or bypass its dependency admission.

The owner requires stopping at any failed gate. Implementation stopped at this result. The failing story and prototype are retained for review. No callback, admission, budget or test was changed to obtain a pass.

### Gate 2: Not Reached — Cancellation Under Backpressure

No cancellation gate was run after Gate 1 failed. The bounded producer and independent control RPC are source only, not proof of responsive cancellation.

### Gate 3: Not Reached — Lifecycle and Fencing

No restart or retired-generation gate was run. Successful authentication and dispatch before the initialization refusal do not establish lifecycle or fencing correctness.

### Gate 4: Not Reached — Slow-Consumer Memory

No memory gate was run. The configured queue and HTTP/2 windows are not a measured bound, and no bounded-memory claim is made.

### Gate 5: Not Reached — Repeated Arrow Throughput

No gRPC or framed Arrow throughput measurements were run because the mandatory stop occurred at Gate 1. Neither carrier has a throughput number from this candidate; no comparison, busy-machine spread or quiet repeat can be claimed. Fast Stream Support remains unproven.

### Gate 6: Not Reached — Net Code Removal

No migration removal estimate was undertaken after the stop. The prototype adds source and removes no production transport. Net code removal is unproven.

## Decision

Do not migrate. Retain the framed carrier, Wasm guests and storage encoding. Any continuation requires owner direction on the failed real-plugin admission before extending the prototype. The SDK-layout proposal remains untouched. README and product-guide shipping claims were reviewed and remain accurate because the experiment is opt-in and no shipping behavior changed.

## Retained Failure Evidence

Base revision: `b59a265342c8068315fec8d2b5cbc1a6ca760f05` from freshly fetched origin/dev. Platform: native Windows x86-64, Rust 1.99.1; the focused process build resolved Tonic 0.14.9. The real gate's exact test filter was `grpc_prototype::gate_1_real_sidecar --exact --nocapture`. An earlier shortened exact filter selected zero tests and is not proof. Compiler-only iterations preceded the gate and are not runtime gate evidence.

The returned error was `Failure`, classification `Permanent`, no refusal code, empty details and no operation/step context. Its diagnostics carried plugin origin and this chain:

```text
plugin lifecycle access failed: plugin operation table failed: operation binding failed: caller may not invoke operation `host.publish` without a host assignment
plugin operation table failed: operation binding failed: caller may not invoke operation `host.publish` without a host assignment
operation binding failed: caller may not invoke operation `host.publish` without a host assignment
caller may not invoke operation `host.publish` without a host assignment
```

The refusal occurred at initialization before the test could execute ListDirectories, its Arrow smoke transfer, shutdown or retirement. Those later assertions were not reached and provide no passing evidence. The process fixture uses kill-on-drop, and no prototype child remained after the failure.

## Final Source Checks

`mise run check:rust` passed: Rust formatting, workspace Clippy, Schematic native Clippy, Schematic Wasm guest Clippy, sandbox Wasm guest Clippy and cargo-machete. The opt-in prototype and focused story also passed their matching Clippy compilation before the runtime gate failed. Runtime proof remains zero passing tests and one failing test; no full test lane or simulation was run. The staged candidate diff scan for the owner's required terms was empty. Successful source checks do not override the failed runtime gate.

## Retention

Native `cargo clean` completed against the three verified worktree-local compiler targets: root (839.8 MiB), Schematic (4.6 GiB), and sandbox (630.7 MiB). No shared cache or other workspace was cleaned. Evidence remains in this tracked design; the worktree is retained and locked at `../harmos.wt/grpc-proto`. The owner authorized branch push only, without a PR or merge.
