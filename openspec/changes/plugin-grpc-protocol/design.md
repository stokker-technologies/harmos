# Plugin gRPC Prototype Design

## Scope and Order

This branch prototypes sidecars only on native Windows, based on the landed unified Invocation over OperationHost. Guests remain Wasm; Postcard remains storage encoding. The existing framed carrier stays unchanged. Migration, harmos-protocol and SDK relocation remain deferred. Gates run in the owner's numbered order, and a gate's own measured failure stops subsequent gates.

The owner corrected the original initialization stop: the missing host.publish assignment was a prototype integration defect, not a failed gate. That refusal is retained below as historical diagnostic evidence. The corrected run passes Gates 1–4 and fails Gate 5 on measured Arrow throughput. Gate 6 is unmeasured.

## Baseline and Environment

Fresh origin/dev is b59a265342c8068315fec8d2b5cbc1a6ca760f05. Continuation started from pushed feat/plugin-grpc-prototype at 0da7c718 in the existing grpc-proto worktree. Fetch followed by merge reported Already up to date; no history rewrite or new worktree was needed.

Measurements ran on 2026-11-06 on Windows 11 Home 10.0.26300, x86-64, AMD Ryzen 9 5950X (16 cores / 32 logical processors), approximately 80 GiB RAM, Rust 1.99.1 and Tonic 0.14.9. Other workers remained active. CPU snapshots during the build and measurement period ranged from 23.63% to 58.65%; two rustc processes and twelve Cargo processes were still present during sampling. These are busy-machine results, not an isolated performance claim. No quiet repeat was available during this run; no other worker was stopped.

The measured bench-profile executable's SHA-256 was 706470a9fc3ed2e4e4a6ff5c25a3f5be639f58171cae5aa162c05b8d945f03ec, recorded before cleanup.

Dokime's stream-ingest-performance design calls for bounded ordered batches, 4,096-row/1 MiB publication ceilings and a 2 ms partial flush policy. Its durable-storage measurements are not transport throughput. This experiment compares identical pre-encoded Arrow IPC batches in both native carriers on the same machine.

## Source Ownership and Prototype Shape

harmos-sidecar owns the opt-in prototype module, Protobuf envelopes, generation admission and client. Schematic owns its experimental executable, process gate story and matched transport measurement. The project Cargo task exposes its already implemented focused run operation through CLI choices; docs/tooling.md documents that entry point. No new task wraps a one-off command.

The separate schematic-grpc-prototype executable listens on an ephemeral loopback address. Its parent supplies a random 256-bit token and generation on private stdin; stdout advertises only the address. Each RPC checks credentials, version and generation. The production entry point and catalog remain unchanged.

Control RPCs dispatch the existing WireTable. Ordinary requests and responses use the SDK Protobuf projection. Initialization now has a dedicated Protobuf message with repeated structured assignments: operation identity, fingerprint and generation. The client translates internal SDK assignments before transmission, and the server recreates their internal representation after receipt. Postcard assignment bytes do not cross this seam. Failures cross as structured JSON bytes preserving classification and diagnostics.

The parent fixture supplies exactly Schematic's declared host.host.publish assignment with the existing Publish fingerprint and generation 1. Normal assignment and callee validation remain active. No publication response is stubbed, no plugin catalog is narrowed, and no broader authority is granted. This path does not invoke publication; host callbacks remain explicitly unsupported. Gate 1 proves the real ListDirectories path, not callback support or a complete migration.

The client implements OperationHost and Instance. Its supported slice is unary operations without host callbacks, retry overrides, portable context values, process-local claims or typed operation endpoints. Unsupported options are refused.

A separate streaming RPC repeats pre-encoded Arrow IPC batches, independently of control calls. Its producer reserves a bounded queue slot before copying a batch and selects cancellation while blocked. The fixture retains the original two queued batches, fixed 64 KiB HTTP/2 windows, 2 MiB message ceiling and 16 concurrent streams. No budget, window, deadline or threshold was tuned after measurement.

## Validation Commands

All compilation and execution use the existing focused mise task, retaining compiler-output preflight and matching Clippy. The process tests use the unoptimized test profile:

    mise run build:rust:focused -- test --locked --manifest-path examples/schematic/Cargo.toml -p schematic-sidecar --features prototype-grpc --test system-prototype -- grpc_prototype::gate_1_real_sidecar --exact --nocapture

The other exact filters are grpc_prototype::gate_2_cancellation_under_backpressure, grpc_prototype::gate_3_restart_and_retirement and grpc_prototype::gate_4_slow_consumer_memory. Local Cargo execution sets the runtime CARGO_BIN_EXE_schematic-grpc-prototype variable to the worktree's examples/schematic/target/debug/schematic-grpc-prototype.exe. The native runner can supply NEXTEST_BIN_EXE_schematic-grpc-prototype.

The optimized transport experiment uses Cargo's unchanged bench profile:

    mise run build:rust:focused -- run --locked --profile bench --manifest-path examples/schematic/Cargo.toml -p schematic-sidecar --features prototype-grpc --bin schematic-grpc-prototype -- --measure

Final proof reruns the four focused stories sequentially and runs mise run check:rust. No full test lane, simulation, PR or integration merge belongs to this slice.

## Gate Results on Windows

### Gate 1: Pass — Real Sidecar Path

The corrected exact process story passed: one test, zero failures, 1.48 seconds. The real unchanged Schematic plugin initializes through Protobuf control, lists actual immediate subdirectories as alpha and beta while excluding an ordinary file, carries three decodable 4,096-row Arrow batches, shuts down through its existing lifecycle operation and retires. The process then physically settles.

### Gate 2: Pass — Cancellation Under Backpressure

One focused story passed in 0.43 seconds. Eight independent 65,928-byte Arrow transfers each requested 65,536 batches. The parent waited for the producer's blocked event, then requested cancellation on the independent control connection without reading or dropping the data response. Each producer settled after four produced batches; subsequent progress stayed unchanged. A fresh transfer completed afterward.

| Repetition | Cancellation Settlement (ms) | Produced Batches |
| --- | ---: | ---: |
| 1 | 0.812 | 4 |
| 2 | 0.712 | 4 |
| 3 | 0.631 | 4 |
| 4 | 0.684 | 4 |
| 5 | 0.600 | 4 |
| 6 | 0.771 | 4 |
| 7 | 0.670 | 4 |
| 8 | 0.708 | 4 |

Measured spread: 0.600–0.812 ms. Correctness assertions use settlement and counters, not a wall-clock performance deadline.

### Gate 3: Pass — Lifecycle and Fencing

One focused story passed in 1.18 seconds. Invalid authored startup configuration produced a structured Permanent failure with plugin origin and the original lifecycle/configuration diagnostic chain; an ordinary operation was unavailable afterward. That physical process retired.

Generation 31 initialized and served the real directory operation. Retirement with a prepared operation and an unread blocked Arrow response fenced a later attempt with FailedPrecondition and refused subsequent ordinary calls. A new physical process at generation 32 refused generation 31's old credential with Unauthenticated and refused a stale generation even with the new credential with FailedPrecondition. Generation 32 initialized, served the real operation, shut down and retired successfully.

This measures explicit physical restart and the prototype's admission fences. It does not claim the shipping runtime's complete automatic restart/reload conformance.

### Gate 4: Pass — Slow-Consumer Memory

One focused story passed in 31.94 seconds. The prototype's existing consumer inputs were unchanged. This gate measures the gRPC process and its parent with Windows Get-Process private memory and working set. Sampling occurs between individual consumed batches, deliberately making the receiver slow. Each payload is warmed once, then requested counts grow from 64 to 4,096 to 65,536; eight slow consumption checkpoints per request retain ordered, byte-identical payloads, followed by cancellation.

Produced minus consumed never exceeded four batches. The fixed pipeline envelope used for the measurement is the existing 2 MiB message ceiling multiplied by the two queued messages plus two in-flight codec positions: 8,388,608 bytes. It was declared before running and was not adjusted afterward.

| Arrow Bytes | Requested Batches | Child Private Min–Max (bytes) | Child Working Set Min–Max (bytes) |
| --- | ---: | ---: | ---: |
| 65,928 | 64 | 3,981,312–4,079,616 | 12,533,760–12,578,816 |
| 65,928 | 4,096 | 3,948,544–4,014,080 | 12,595,200–12,607,488 |
| 65,928 | 65,536 | 3,928,064–4,026,368 | 12,595,200–12,660,736 |
| 1,048,968 | 64 | 8,921,088–8,921,088 | 17,580,032–17,580,032 |
| 1,048,968 | 4,096 | 9,195,520–9,195,520 | 17,645,568–17,645,568 |
| 1,048,968 | 65,536 | 9,293,824–9,486,336 | 17,653,760–17,702,912 |

Child private-memory growth above warm baseline was 581,632 bytes for the smaller payload and 5,840,896 bytes for the larger. Parent growth was 638,976 and 2,576,384 bytes, respectively. All stayed within the fixed measurement envelope. These sampled plateaus and bounded producer counters support this fixture's bound; they are not a universal allocator/RSS bound for a future complete SDK.

### Gate 5: Fail — Repeated Arrow Throughput

The optimized executable completed both distributions and returned exit code 1 for measured throughput regression. A conservative no-regression criterion was declared before execution: gRPC median useful Arrow throughput must be at least the framed median for both payload sizes. This is an experiment decision rule, not a newly asserted owner-provided numeric throughput budget.

Both children run the same optimized executable and multi-thread Tokio runtime. The baseline uses today's unmodified framed Connection, actual stdin/stdout process pipes, native authenticated independent control pipe, existing credits and writer. Its experimental producer repeats the same input bytes in Answer frames; the gRPC producer repeats them in Packet messages. Neither path runs storage, Arrow encoding or per-batch content verification inside timing. Order/count/byte accounting occurs during receipt; the final batch and Arrow schema/row content are verified outside timing. Timing includes the transfer request, complete receiver consumption and a completion acknowledgment. The result isolates transport rather than the durable ingest pipeline.

Payloads contain UInt64 sequence and Float64 value columns. Each trial transfers approximately 256 MiB: 4,072 batches of 65,928 bytes or 256 batches of 1,048,968 bytes. Each size has an eight-batch warmup, then seven repetitions alternating which carrier runs first. The original gRPC settings and framed transport budgets remain unchanged.

| Arrow Bytes | Repetition | gRPC (MiB/s) | Framed Stdio (MiB/s) |
| --- | ---: | ---: | ---: |
| 65,928 | 1 | 30.121 | 662.580 |
| 65,928 | 2 | 28.123 | 570.064 |
| 65,928 | 3 | 28.550 | 330.811 |
| 65,928 | 4 | 28.796 | 364.919 |
| 65,928 | 5 | 29.358 | 361.581 |
| 65,928 | 6 | 27.623 | 393.845 |
| 65,928 | 7 | 31.548 | 508.828 |
| 1,048,968 | 1 | 92.150 | 1,312.957 |
| 1,048,968 | 2 | 88.507 | 1,345.456 |
| 1,048,968 | 3 | 98.310 | 1,224.554 |
| 1,048,968 | 4 | 88.060 | 1,313.049 |
| 1,048,968 | 5 | 82.837 | 1,429.598 |
| 1,048,968 | 6 | 82.436 | 1,409.121 |
| 1,048,968 | 7 | 87.832 | 1,436.724 |

| Arrow Bytes | gRPC Median [Min, Max] (MiB/s) | Framed Median [Min, Max] (MiB/s) | gRPC / Framed Median |
| --- | ---: | ---: | ---: |
| 65,928 | 28.796 [27.623, 31.548] | 393.845 [330.811, 662.580] | 0.0731 |
| 1,048,968 | 88.060 [82.436, 98.310] | 1,345.456 [1,224.554, 1,436.724] | 0.0654 |

gRPC achieves about 7.3% and 6.5% of the framed medians. The best gRPC trial is below the slowest framed trial for both payloads despite the busy-machine spread. Startup-to-ready observations outside throughput timing were 38.037 ms for gRPC and 32.441 ms for framed stdio; these single observations are not distributions.

Fast Stream Support is not demonstrated by this prototype. The failure is specific to the retained implementation and settings; it does not establish an intrinsic ceiling of all possible gRPC implementations. No tuning or workaround followed this measurement.

### Gate 6: Unmeasured — Net Code Removal

The ordered stop occurred at Gate 5. No migration deletion/addition estimate was made, and no net-removal claim is made. Source retained by the prototype is not itself a migration estimate.

## Decision and Documentation Impact

Do not migrate on this evidence. Preserve the existing framed carrier, Wasm guests and Postcard storage. Any further prototype tuning or migration requires new owner direction; this slice stops at the measured throughput failure.

README.md and docs/product-guide.md shipping claims remain accurate and unchanged: this opt-in executable does not replace a shipped carrier. docs/tooling.md documents the focused Cargo selection and optimized measurement command. The SDK-layout proposal remains untouched.

## Historical Integration Diagnostic

The first execution on the previous checkpoint stopped during Schematic initialization after 3.05 seconds with:

    caller may not invoke operation host.publish without a host assignment

The structured Permanent failure preserved plugin origin and the lifecycle/table/binding diagnostic chain. Initialization used an empty assignment list, so later assertions were never reached. An earlier shortened exact filter selected zero tests and was not proof. The owner explicitly classified the missing assignment as an integration defect; it is not counted as a gate failure. The corrected structured assignment boundary and successful Gate 1 supersede that stop decision.

## Final Validation and Retention

The individual ordered proofs passed Gates 1–4. The optimized Gate 5 measurement completed and failed on its own numbers; Gate 6 remains unmeasured. The final focused rerun on the completed source passed all four stories sequentially: four passed, zero failed, 30.63 seconds of test execution. Its repeated cancellation settlement was 0.478–0.774 ms. Child private-memory growth was 471,040 / 5,730,304 bytes and parent growth was 344,064 / 2,138,112 bytes for the smaller/larger payloads; the original fixed envelope remained unchanged. No full simulation or full test lane ran.

mise run check:rust passed: Rust formatting; workspace Clippy; Schematic native Clippy; Schematic Wasm guest Clippy; sandbox Wasm guest Clippy; and Cargo Machete with no unused dependencies. The opt-in test configuration and optimized measurement binary passed their own matching Clippy selections through the focused task. mise run format:python --check passed for the one-line task CLI repair. The full staged candidate diff against origin/dev passed the owner's required case-insensitive term scan with no matches, and git diff --check was empty. Source checks do not override Gate 5's measured failure.

Evidence is retained in this tracked design before native cleanup. Only verified worktree-local compiler targets may be cargo-cleaned. The existing worktree is kept, and delivery is branch push only, without PR or integration merge.
