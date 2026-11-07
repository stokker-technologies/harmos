# Plugin gRPC Prototype Design

## Scope and Order

The owner's latest 2026-11-07 decision is: "i am satisfied with the grpc results, i would try to minimize the machinery needed in the sdks. Above 200 MiB/s is good for now." It supersedes the earlier ordered tuning authorization and Gate 5's no-regression median rule. The current shape removes application consumption credits and the bidirectional Data RPC, restores the 64 KiB stream window and retains every other b12c8716 setting. Gate 5 requires the gRPC median to exceed 200 MiB/s on both unchanged payloads. Framed remains measured in the same seven alternating paired repetitions and reported without deciding the gate. A sub-floor median with mean timed CPU above 80% is inconclusive pending the orchestrator's quiet-machine rerun. Gates 2–4 retain every assertion. No further tuning, copy removal, named pipes, migration, PR, dev merge or Dokime change is authorized.

This branch prototypes sidecars on native Windows, based on the landed unified Invocation over OperationHost. Existing Linux infrastructure may be assessed without provisioning. Guests remain Wasm; Postcard remains storage encoding. The existing framed carrier stays unchanged. Migration, harmos-protocol and SDK relocation remain deferred. Earlier gate rules and outcomes below are historical checkpoints; the latest owner decision controls this continuation.

The owner corrected the original initialization stop: the missing host.publish assignment was a prototype integration defect, not a failed gate. That refusal is retained below as historical diagnostic evidence. The original corrected run passes Gates 1–4 and fails Gate 5 on measured Arrow throughput. The owner subsequently authorized the single bounded tuning continuation below, including Gate 6 despite the original Gate 5 stop. Migration and any merge remain unauthorized.

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

A separate streaming RPC repeats pre-encoded Arrow IPC batches, independently of control calls. Its producer reserves a bounded queue slot and selects cancellation while blocked. The original checkpoint copied each batch and used two queued batches, fixed 64 KiB HTTP/2 windows, a 2 MiB message ceiling and 16 concurrent streams. No tuning followed that original measurement until the owner authorized the bounded continuation recorded below.

## Validation Commands

All compilation and execution use the existing focused mise task, retaining compiler-output preflight and matching Clippy. The process tests use the unoptimized test profile:

    mise run build:rust:focused -- test --locked --manifest-path examples/schematic/Cargo.toml -p schematic-sidecar --features prototype-grpc --test system-prototype -- grpc_prototype::gate_1_real_sidecar --exact --nocapture

The other exact filters are grpc_prototype::gate_2_cancellation_under_backpressure, grpc_prototype::gate_3_restart_and_retirement and grpc_prototype::gate_4_slow_consumer_memory. Local Cargo execution sets the runtime CARGO_BIN_EXE_schematic-grpc-prototype variable to the worktree's examples/schematic/target/debug/schematic-grpc-prototype.exe. The native runner can supply NEXTEST_BIN_EXE_schematic-grpc-prototype.

The optimized transport experiment uses Cargo's unchanged bench profile:

    mise run build:rust:focused -- run --locked --profile bench --manifest-path examples/schematic/Cargo.toml -p schematic-sidecar --features prototype-grpc --bin schematic-grpc-prototype -- --measure

Final proof reruns the four focused stories sequentially and runs mise run check:rust. No full test lane, simulation, PR or integration merge belongs to this slice.

## Original Gate Results on Windows

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

### Gate 6: Original Stop — Net Code Removal

The original ordered stop occurred at Gate 5. The newly authorized estimate appears in the bounded continuation below; no migration has landed.

## Decision and Documentation Impact

Do not migrate on the original evidence. Preserve the existing framed carrier, Wasm guests and Postcard storage. The owner authorizes exactly one subsequent profiling/tuning pass and a branch push, with no PR, merge, migration or second tuning round. The original gate criterion remains unchanged.

README.md and docs/product-guide.md shipping claims remain accurate and unchanged: this opt-in executable does not replace a shipped carrier. docs/tooling.md documents the focused Cargo selection and optimized measurement command. The SDK-layout proposal remains untouched.

## Bounded Tuning Continuation

### Authorization and Baseline

On 2026-11-07 the owner authorized one short profiling/tuning pass, repeated matched Gate 5 measurements, retained Gates 2–4 and a Gate 6 estimate. No gate threshold, payload, framed setting, migration, PR or integration merge may change. The clean owned worktree started at b0f26cea on feat/plugin-grpc-prototype, tracking its same-named origin branch. Fresh origin/dev at 22c3b40587110e41a5c16c7849ebe28b5dc88322 merged normally before any edits. Nothing was pushed to dev; published history is not rewritten.

### Profile Before Tuning

Before changing transport behavior, instrumentation measured the existing optimized stream with the same 65,928-/1,048,968-byte payloads and 4,072/256 batches. HARMOS_PROFILE_TRANSPORT=1 selects one diagnostic repetition, not the seven-repetition Gate 5 proof. It times Protobuf encode/decode and producer payload cloning/reservation, observes actual accepted-socket NODELAY and counts H2 capacity events through a filtered tracing subscriber. These overlapping thread measurements must not be added as a wall-clock partition; enabled tracing also perturbs this diagnostic run.

| Arrow Bytes | Producer Elapsed (ms) | Queue Reservation Wait (ms) | Payload Clone (ms) | Server Encode (ms) | Client Decode (ms) |
| --- | ---: | ---: | ---: | ---: | ---: |
| 65,928 | 9,053.472 | 9,015.808 | 30.922 | 63.947 | 68.115 |
| 1,048,968 | 3,678.939 | 3,556.689 | 121.721 | 214.203 | 147.341 |

The producer is blocked on its downstream consumer for 99.6% / 96.7% of its elapsed time, rather than starved for input or dominated by Arrow encoding. Arrow IPC is already encoded before timing in both carriers. The original Vec payload is cloned per batch, copied into Tonic's contiguous Protobuf encoding buffer, then copied into a decoded Vec. The first copy and codec work are measurable but much smaller than the wait. H2's prioritize trace observed buffered data with exhausted connection capacity, and later both stream and connection windows exhausted; its cumulative counter reached 4,096 such events. This counts observations, not distinct stalls or their duration.

Both actual accepted server sockets reported NODELAY=false. The client Endpoint defaults to true; the custom server incoming stream bypassed that default. There is no TLS setup, certificate negotiation, encryption or compression in this experiment. These observations locate the dominant wait downstream in HTTP/2/TCP, without claiming a sampled CPU-stack or packet-capture separation of kernel TCP time from H2 credit turnaround. The diagnostic rates were gRPC/framed 28.259/332.331 and 68.763/1,142.470 MiB/s. CPU samples around execution were 31–85%, with six to seven Cargo and three to four rustc processes present; this was not quiet.

### Single Selected Tuning

- Set TCP_NODELAY on each accepted socket itself, and explicitly on the client. Tonic's Server setter alone is ignored by serve_with_incoming ([Tonic 0.14.9 Server](https://docs.rs/tonic/0.14.9/tonic/transport/struct.Server.html)).
- Increase each endpoint's initial connection window from 64 KiB to 2 MiB; keep its stream window at 64 KiB. This removes the connection-level bottleneck without allowing the single data stream to run ahead past the original bound. Adaptive windows remain off: adaptive expansion or a larger stream window would change the amount of unread data admitted and could invalidate Gate 4. No application-credit protocol is added to work around that constraint.
- Advertise 64 KiB maximum H2 frames on both endpoints, increase initial codec allocation from 8 to 64 KiB, retain its one-message yield threshold, and bound the client's request buffer to the existing 16-call admission limit. Message size stays 2 MiB, producer queue stays two, and concurrent streams stay 16. Socket send/receive buffers retain OS defaults; no unsupported socket tuning is introduced.
- Change only the prototype Packet payload to prost bytes::Bytes. Taking ownership of the request Vec and cloning a queued batch now share immutable bytes; Protobuf decoding slices Tonic's BytesMut through DecodeBuf::copy_to_bytes. Arrow is never re-encoded. This is zero-copy payload ownership/decoding, not a claim of end-to-end zero-copy: the standard Tonic Encoder API still writes payload bytes into a contiguous EncodeBuf for each message. Removing that last copy would require a different body/codec integration, outside this pass.

These settings are one selected bundle, not an isolated attribution experiment for every knob. Framed producer, framing, credits, control pipe, timing, count and payload bytes remain unchanged. Tonic's installed transport code has Connected implementations for TCP and Unix streams, but none for Windows named pipes. A named-pipe experiment would need an owned IO/Connected wrapper plus connector, listener and lifecycle handling; no clean built-in Windows named-pipe switch exists, so it was not attempted in this bounded pass.

### Retained Gates 1–4

The focused compiler check selected both the prototype executable and system-prototype test. Matching focused Clippy and unoptimized Cargo tests then ran all four grpc_prototype:: stories sequentially: four passed, zero failed, 45.21 seconds. Gate 1 retains real initialization, directory listing, Arrow verification and shutdown. Gate 2's eight unread-response cancellations settled with exactly four produced batches and no later progress, in 0.601–1.199 ms; a new transfer still completed. Gate 3 retains structured startup failure, physical restart, prepared-call retirement, credential/generation fences and shutdown.

Gate 4 retains all original assertions, requested counts 64/4,096/65,536, eight slow-consumer checkpoints and both payloads. Produced minus consumed never exceeded four. The existing two-message queue and two in-flight positions still use the original 8,388,608-byte allowance; the larger connection window did not weaken this bound.

| Arrow Bytes | Child Private Baseline / Peak / Growth (bytes) | Parent Private Baseline / Peak / Growth (bytes) |
| --- | ---: | ---: |
| 65,928 | 3,629,056 / 4,186,112 / 557,056 | 1,720,320 / 2,011,136 / 290,816 |
| 1,048,968 | 3,694,592 / 6,934,528 / 3,239,936 | 2,797,568 / 4,087,808 / 1,290,240 |

### Gate 5 After Tuning: Fail

The same machine, Rust 1.99.1, Tonic 0.14.9 and unchanged Cargo bench profile ran the tuned diagnostic and then one seven-repetition paired proof, with profiling disabled for that proof. Both optimized child carriers ran in the same invocation, alternating order, with the unchanged eight-batch warmups, 4,072/256 timed batches, exact payload sizes and approximately 256 MiB per trial. No storage, Arrow re-encoding or timed content comparison was added. The framed function and producer have no changes. No project validation ran concurrently with measurement.

| Arrow Bytes | Repetition | gRPC (MiB/s) | Framed Stdio (MiB/s) |
| --- | ---: | ---: | ---: |
| 65,928 | 1 | 244.343 | 410.787 |
| 65,928 | 2 | 208.996 | 369.935 |
| 65,928 | 3 | 249.141 | 350.488 |
| 65,928 | 4 | 329.805 | 402.791 |
| 65,928 | 5 | 320.530 | 460.183 |
| 65,928 | 6 | 357.613 | 440.108 |
| 65,928 | 7 | 241.084 | 410.025 |
| 1,048,968 | 1 | 404.013 | 1,009.716 |
| 1,048,968 | 2 | 342.488 | 856.414 |
| 1,048,968 | 3 | 366.769 | 933.598 |
| 1,048,968 | 4 | 322.092 | 776.497 |
| 1,048,968 | 5 | 320.829 | 853.903 |
| 1,048,968 | 6 | 352.312 | 726.742 |
| 1,048,968 | 7 | 371.454 | 1,123.146 |

| Run | Arrow Bytes | gRPC Median [Min, Max] (MiB/s) | Framed Median [Min, Max] (MiB/s) | gRPC / Framed Median |
| --- | ---: | ---: | ---: | ---: |
| Original | 65,928 | 28.796 [27.623, 31.548] | 393.845 [330.811, 662.580] | 0.0731 |
| Tuned | 65,928 | 249.141 [208.996, 357.613] | 410.025 [350.488, 460.183] | 0.6076 |
| Original | 1,048,968 | 88.060 [82.436, 98.310] | 1,345.456 [1,224.554, 1,436.724] | 0.0654 |
| Tuned | 1,048,968 | 352.312 [320.829, 404.013] | 856.414 [726.742, 1,123.146] | 0.4114 |

The tuned gRPC medians are 8.65 and 4.00 times the historical gRPC medians, but neither meets the original no-regression rule. The paired comparison is the valid current ratio; the historical comparison is not a controlled cross-day speedup claim. Framed also varies substantially with machine load. Gate 5 completed and returned exit code 1 for measured regression; this is not a compiler/test failure. No threshold or new target number was introduced.

The paired task ran from 07:14:50.803 to 07:15:14.678 CET on 2026-11-07. Ten CPU snapshots during that task ranged from 41% to 70%, averaging 57%. Other compiler jobs remained present; no unrelated process was stopped. This was the available paired run after this task's own test compilation had settled, but it was still a busy machine, not a quiet or isolated benchmark. Startup observations outside timed throughput were 21.029 ms gRPC and 32.711 ms framed, not latency distributions. The profile-only pre-tuning executable SHA-256 was ba306259ddaad4ad6fefd1fbb57a0671adb71fb6e4265e8cc3e2fd77bdcbee17; the tuned executable used by both diagnostic and seven-repetition proof was 09ffb9a41bb36f02fcd627aac49afe17ba88c2393a578fe8350b2f5cdaeb4eb0.

### Remaining Bottleneck and Recommendation

The post-tuning diagnostic confirms NODELAY=true on both accepted sockets. Its first buffered-data capacity observation has stream window zero while connection capacity is 1,966,080 bytes: increasing the connection window removed that particular constraint, leaving the fixed stream window's credit turnaround. Producer elapsed/wait times are 1,068.372/1,065.185 ms for the smaller payload and 799.412/798.911 ms for the larger. Payload clone time falls to 0.148/0.011 ms, and client decoding to 4.869/0.604 ms; server encoding still takes 31.939/121.111 ms. The Bytes change removes the measured clone/decode copying costs, but the contiguous Tonic output copy and H2/TCP scheduling/credit overhead remain. The settings bundle was not decomposed into separate causal speedup measurements, so the gain cannot all be attributed to NODELAY, frames or Bytes individually.

The remaining large gap is this bounded Tonic stream implementation's flow-control/transport path, with encoding cost especially visible for the larger batch. It is not evidence of an inherent gRPC throughput ceiling: gRPC does not mandate a 64 KiB stream window or Tonic's contiguous-buffer codec. Larger/adaptive stream windows would require revisiting how unread buffering is bounded or adding application credits; neither is attempted here, and Gate 4 is not weakened. A CPU-stack/ETW/packet trace would be needed to subdivide residual kernel TCP and H2 scheduler time precisely. No second round is started.

Recommendation: retain the shipping framed carrier and do not migrate on this evidence. If language-neutral control justifies a future hybrid, use gRPC for lifecycle/operations/cancellation and retain the framed Arrow path for stream bytes, with explicit cross-channel invocation identity, credits, generation authentication, ordering, cancellation settlement and teardown. The dual contract and SDK/conformance costs are itemized in Gate 6; the owner decides whether the measured ratio and interoperability benefit justify them. This pass implements no hybrid and changes no agreed migration contract. README.md and docs/product-guide.md still describe shipping behavior accurately; the prototype remains opt-in and the bounded evidence belongs here.

### Gate 6: Estimated Net Code Removal

The estimate audits current owners, not a migration diff. Counts are physical Rust source lines including comments and blanks; generated code, tests, manifests and docs are excluded from both sides. The following are optimistic whole-file removal candidates for a complete gRPC replacement:

| Owner | Files | Candidate Lines |
| --- | --- | ---: |
| Framing and Credit | crates/harmos-sidecar/src/transport.rs and transport/{connection,credit,consumption}.rs | 1,151 |
| Frame Model and Wire Conversion | crates/harmos-sidecar/src/wire.rs | 454 |
| Carrier Adapters | crates/harmos-sidecar/src/kit/{carrier,connection}.rs and crates/harmos-runtime/src/services/sidecar/carrier.rs | 303 |
| Independent Control Bootstrap | crates/harmos-sidecar/src/attach/{control,windows}.rs | 284 |
| Total Candidate Removal | | 2,192 |

The original three-file prototype already occupies 908 lines, including its framed measurement helper and scaffolding. It is not a complete replacement: host callbacks/publications, actual typed upload/download/exchange streams, observers, retry/context/claims behavior, bidirectional cancellation/settlement and automatic supervision integration remain to implement. Lifecycle, admission, generation fencing, process management and common SDK WireTable/guest behavior must remain; counting those files as deletions would exaggerate savings. A planning estimate is 1,900–2,900 replacement Rust lines after excluding the measurement helper and adding the missing behavior. This is an engineering range, not measured implementation size: it yields about 292 net lines removed at the optimistic end through 708 net lines added. Net removal is not established. Authored .proto, conformance and new language SDK source would add further work outside this Rust-only estimate. Existing behavior would need a full inventory and conformance proof before narrowing this range.

A hybrid retains the 1,605 framing/wire lines and most carrier adapters, while adding gRPC control/bootstrap and coordination, so it offers no credible net-removal advantage. C++ and Go would each implement generated gRPC control plus the framed stream codec, Arrow IPC batch handling, credit accounting, ordered invocation/stream correlation, cancellation/settlement and EOF/error handling. They would also need one authenticated generation identity across both channels, teardown/restart fencing and shared conformance cases for cross-channel races. Protobuf generation alone would not constitute either SDK. This is a cost assessment only; the existing proposed one-protocol migration contract is not silently changed to a hybrid.

### Bounded Validation and Retention

mise run check:rust passed on the completed source: Rust formatting for all three workspaces, workspace Clippy including tests, Schematic native CLI/sidecar Clippy, Schematic Wasm guest Clippy, sandbox Wasm guest Clippy and Cargo Machete with no unused dependencies. The prototype executable and process tests also passed their exact matching focused Clippy selections. The combined candidate diff against origin/dev and the continuation diff against b0f26cea both passed the required case-insensitive term scan with no matches; git diff --check was empty. Focused tests remain four passed, zero failed; no full lane, CI simulation or Linux proof is claimed. Gate 5 remains a completed measured failure. Gate 6 is an estimate rather than a migration result.

The bounded source and evidence are complete. Delivery is limited to normal pushes of feat/plugin-grpc-prototype, with no PR or merge into dev. The retained worktree lock remains Owner requested retained gRPC prototype. No migration or second tuning round is started, and the deferred migration tasks remain open.

The source/evidence checkpoint a4c36cc27654e9570d9480f85701dc0ff7e6f074 was pushed normally and verified against the remote SHA. After preserving the distributions, focused proof, check results and executable digests here, native cargo clean completed through mise's tool environment for all three Cargo-metadata-resolved targets, each checked to be non-linked and inside this owned worktree: root 751.3 MiB, Schematic 5.7 GiB and sandbox 630.7 MiB. No peer target or shared compiler cache was cleaned. All prototype children had settled. Only this cleanup record and task completion bookkeeping changed after the validated source checkpoint; the worktree remains present and locked.

## Ordered Tuning Continuation

This section retains the credits experiment as history. Its no-regression criterion, wider window, application receipts, SDK costs and stop decision describe that checkpoint only; the owner-directed SDK simplification below supersedes them.

### Current-Load Baseline and Method

This continuation starts from the clean pushed b12c87164416527eae8517db186a85c48ba5ff5d checkpoint in the same owned worktree and tracking branch. Fetch does not change the candidate; no dev merge or history rewrite is performed. The owner-provided prior medians remain 249.141/410.025 MiB/s for 65,928 bytes and 352.312/856.414 MiB/s for 1,048,968 bytes. A fresh unchanged-source baseline measures current machine contention before the first separately attributed change. Only owned implementation edits were temporarily saved and restored around that run.

Every performance change uses the existing optimized focused mise command above, the same executable for both carriers, unchanged payloads/counts/framed implementation, eight-batch warmups and seven repetitions alternating carrier order. Gate 5 still requires gRPC's median to meet or exceed framed's median on both payloads. A measured Gate 5 failure returns exit code 1 and does not masquerade as a compiler or correctness failure. Raw median gains between changes are reported alongside paired ratios; changing machine contention limits causal precision even with alternating pairs. No other worker is stopped.

CPU load is sampled approximately every 0.5 seconds with Windows GetSystemTimes. Only samples from GATE_5_STARTUP through GATE_5_RESULT enter the reported range/mean; compilation samples are excluded. Ignored logs and CPU samples live in target/benchmarks/grpc-continuation until cleanup; durable distributions and conclusions are retained here.

| Checkpoint | Arrow Bytes | gRPC Median [Min, Max] (MiB/s) | Framed Median [Min, Max] (MiB/s) | Paired Median Ratio |
| --- | ---: | ---: | ---: | ---: |
| Fresh Baseline | 65,928 | 187.301 [175.018, 213.781] | 259.371 [218.816, 277.658] | 0.7221 |
| Fresh Baseline | 1,048,968 | 218.502 [174.568, 254.202] | 460.365 [357.867, 680.975] | 0.4746 |

The fresh baseline has 44 timed CPU samples, range 66.70–100.00%, mean 96.77%. Both payloads still fail Gate 5. The busy machine slowed both carriers relative to the owner's prior run; it is essential to retain each contemporaneous framed comparison.

### Stream Window and Consumption Credits

The first change raises both peers' fixed stream windows from 64 KiB to 2 MiB while retaining the already tuned 2 MiB connection window and all other transport settings. A larger HTTP/2 window cannot itself preserve the old queue-derived unread bound. The prototype Data method therefore becomes bidirectional: its first Packet is the existing request, subsequent phase-13 Packets acknowledge cumulative delivered batch counts. A semaphore starts with queued_batches + 2 credits (four in this fixture); every produced batch consumes one, and only a monotonic same-generation/same-invocation receipt no greater than actual production replenishes it. Credits are independent of Tonic/h2 receive buffering. Receipt EOF/error and control cancellation settle the producer without waiting for data capacity.

The receiving wrapper acknowledges a batch when message() delivers it; consumers retaining batches afterward own that memory just as with the framed consumption contract. Gate 4 still samples before the next read, retains the original produced-minus-consumed assertion and the original 8,388,608-byte memory envelope. The producer, queue and receipt reader all remain bounded. C++ and Go must implement this application receipt contract and its lifecycle tests; generated Protobuf alone is insufficient.

### Separately Measured Window Result and Stop

| Arrow Bytes | Repetition | Baseline gRPC | Baseline Framed | Window + Credits gRPC | Window + Credits Framed |
| --- | ---: | ---: | ---: | ---: | ---: |
| 65,928 | 1 | 195.637 | 249.651 | 336.204 | 231.711 |
| 65,928 | 2 | 175.018 | 277.658 | 366.159 | 280.513 |
| 65,928 | 3 | 213.781 | 271.514 | 268.219 | 203.245 |
| 65,928 | 4 | 187.301 | 218.816 | 279.592 | 194.876 |
| 65,928 | 5 | 193.370 | 231.032 | 237.098 | 89.734 |
| 65,928 | 6 | 176.932 | 259.371 | 352.592 | 174.936 |
| 65,928 | 7 | 181.665 | 268.400 | 356.061 | 179.285 |
| 1,048,968 | 1 | 174.568 | 395.477 | 445.855 | 832.532 |
| 1,048,968 | 2 | 218.291 | 357.867 | 381.923 | 850.515 |
| 1,048,968 | 3 | 198.069 | 680.975 | 351.706 | 273.525 |
| 1,048,968 | 4 | 218.502 | 609.652 | 277.237 | 335.140 |
| 1,048,968 | 5 | 242.589 | 421.717 | 413.357 | 372.417 |
| 1,048,968 | 6 | 254.202 | 460.365 | 381.553 | 282.035 |
| 1,048,968 | 7 | 234.612 | 486.825 | 408.255 | 458.233 |

All throughput entries are useful Arrow MiB/s. Repetition numbers identify order within each separate checkpoint, not simultaneous before/after pairs.

| Arrow Bytes | Window + Credits gRPC Median [Min, Max] (MiB/s) | Unchanged Framed Median [Min, Max] (MiB/s) | gRPC / Framed | gRPC Gain over Fresh Baseline |
| --- | ---: | ---: | ---: | ---: |
| 65,928 | 336.204 [237.098, 366.159] | 194.876 [89.734, 280.513] | 1.7252 | +79.50% |
| 1,048,968 | 381.923 [277.237, 445.855] | 372.417 [273.525, 850.515] | 1.0255 | +74.79% |

The fresh baseline timed interval is 07:34:36.574–07:35:05.559 CET on 2026-11-07; the window/credits interval is 07:42:07.203–07:42:32.813. The latter has 45 timed CPU samples, range 67.02–100.00%, mean 97.22%. Both carriers' broad distributions reflect active competing workers. The framed source, settings, byte counts and timing boundaries were not changed or tuned. Framed's median decreased 24.87% / 19.10% between checkpoints, while its 1 MB maximum more than doubled, so the raw gains cannot isolate an exact causal effect under this load. Relative to the owner's older medians, gRPC improved 34.95% / 8.40%; those are historical comparisons, not separately controlled gains. No outlier was removed.

Gate 5's unchanged median rule returns PASS for both payloads. The first owner-required stop condition therefore fires after the stream-window/credits change. Copy removal, Windows named pipes, Unix sockets and new CPU/ETW profiling are not started. All reported gRPC measurements are TCP; there is no pipe result or inferred Unix result. The narrower 1 MB margin is only 2.55%, and several individual gRPC trials lose to framed. Passing this declared busy-machine median rule does not establish a portable or isolated no-regression guarantee. No extra threshold or confidence requirement is substituted for the owner's gate.

The measured optimized executable SHA-256 is 50328963768f5e6dd8b58c03144dfd6843e51e205890fe83a6ab87bf1431ecdc. The only behavioral change for this checkpoint is the stream window plus the credits needed to preserve Gate 4. NODELAY, connection window, frame size, codec and framed baseline retain the previous checkpoint's behavior.

### Retained Correctness Proof

The existing focused process target ran with filter grpc_prototype:: and --test-threads=1 through build:rust:focused, including its matching Clippy. Four tests passed, zero failed, in 68.36 seconds: real plugin initialization/operation, cancellation under unread backpressure, physical restart/generation retirement and slow-consumer memory. No test assertion, payload, gate limit or framed budget changed. Gate 2 settled all eight unread producers at four batches, with cancellation observations 0.777–2.231 ms; a fresh transfer completed afterward. Gate 3 retained structured startup refusal, prepared-call fencing, old credential refusal, stale-generation refusal and shutdown settlement.

| Arrow Bytes | Child Baseline / Peak / Growth (bytes) | Parent Baseline / Peak / Growth (bytes) | Unchanged Allowance (bytes) |
| --- | ---: | ---: | ---: |
| 65,928 | 3,747,840 / 4,042,752 / 294,912 | 1,871,872 / 2,633,728 / 761,856 | 8,388,608 |
| 1,048,968 | 4,919,296 / 8,048,640 / 3,129,344 | 3,739,648 / 8,101,888 / 4,362,240 | 8,388,608 |

Gate 4 again exercised requested counts 64, 4,096 and 65,536 for both unchanged payloads, eight slow checkpoints each. Produced minus consumed never exceeded four. The wider window did not change either memory assertion or the allowance formula. These remain process-fixture observations rather than a universal allocator guarantee.

### Historical Credits Gate 6 and SDK Cost

Recounting the three current prototype owner files gives 545 lines in crates/harmos-sidecar/src/kit/prototype.rs, 364 in prototype/client.rs and 165 in prototype/codec.rs: 1,074 physical Rust lines. The pushed pre-continuation source actually had 982, so consumption credits and the bidirectional wrapper add 92 net lines. This corrects the earlier 908-line statement, which did not include all later instrumentation. The 2,192-line optimistic removal inventory above is unchanged and has been recounted against current files.

Adding approximately 100 lines of credit ownership to the earlier replacement range revises Gate 6 to about 2,000–3,000 replacement Rust lines: at best 192 net lines removed, through 808 net lines added. This is still a planning estimate, not a migration result or evidence of net code removal. It excludes generated code, tests, manifests, docs and new language SDKs on both sides, and still needs the previously listed callbacks, typed streams, invocation options, observer and supervision behavior. No custom output codec or pipe adapter is included because neither was implemented.

C++ and Go would each need a bidirectional Data RPC, an initial request followed by cumulative consumption receipts, producer credit acquisition before production, validation of monotonic counts against production and invocation/generation identity, bounded outgoing receipt delivery, cancellation/EOF settlement and conformance cases for races and unread consumers. The four-credit fixture allowance derives from its existing queue plus two in-flight positions; a complete SDK must retain consumer ownership of that budget. Each implementation must also map the 2 MiB receive-window intent to its own gRPC settings and remeasure. Roughly 100–200 additional authored lines per language for credit behavior is a planning allowance, excluding generated declarations and conformance tests; it is not a measured SDK port. None of this requires a Rust-only wire trick, but only the Rust/Tonic implementation has been tested. Ordinary generated gRPC clients without receipt behavior will stop after exhausting credits.

### Remaining Costs and Recommendation

Tonic's contiguous output copy and H2/TCP framing, credit traffic, scheduling and wake-ups remain. The prior profile locates these costs but does not partition the final checkpoint's CPU time. The output copy is a Tonic implementation choice, and the old 64 KiB window was a setting; neither is established as inherent to gRPC. This continuation makes no new claim of an irreducible gRPC/h2 bottleneck because the successful-gate stop preceded any residual profile.

Keep the shipping framed carrier while retaining this successful bounded TCP prototype. Gate 5 now passes its declared Windows median rule, but Gate 6 still does not establish net removal, complete SDK behavior and Linux proof remain deferred, and the busy-machine result has little 1 MB margin. README.md and docs/product-guide.md continue to describe shipping behavior accurately, so their claims and examples are unchanged. No migration, hybrid, Dokime modification, PR or merge is implemented or authorized by this continuation.

### Final Checks and Branch Retention

The completed source passed mise run check:rust: all three workspace formatting selections; workspace Clippy including tests; Schematic native CLI/sidecar Clippy; Schematic Wasm guest Clippy; sandbox Wasm guest Clippy; and Cargo Machete with no unused dependencies. The opt-in prototype's focused process and optimized measurement selections also passed matching Clippy. openspec validate plugin-grpc-protocol --strict passed and git diff --check was empty. There were no compiler allocation failures or source/test/check failures; the unchanged baseline's Gate 5 exit code 1 was its measured throughput failure, followed by the tuned checkpoint's Gate 5 exit code 0. No full test lane, CI simulation or Linux proof is claimed. Delivery remains a normal push of this prototype branch, followed by native Cargo cleanup and retention of the existing worktree.

The source/proof checkpoint 30b450cb42c41ee2f8377b65833adc39ab03b025 was pushed normally to feat/plugin-grpc-prototype after the owner's exact bash scan printed CLEAN, then verified against the remote SHA. The final documentation-only follow-up records completed delivery and cleanup and receives the same scan before its push. No PR or merge was created.

All three Cargo-metadata-resolved target paths were verified inside this worktree, with no linked path components or retained CI evidence, and all owned prototype children had settled. Native cargo clean initially refused the root target because it lacked CACHEDIR.TAG: the measurement harness had created that directory for evidence before Cargo. Profile/workspace-scoped native cleanup had the same refusal. Raw benchmark logs and CPU samples were moved into this task's system-temp scratch directory; the remaining root inventory contained only Cargo's debug, tmp and .rustc_info.json outputs. After restoring the standard Cargo-created cache tag from this worktree's tagged Schematic target to that verified compiler-cache root, native cargo clean completed for root (751.3 MiB), Schematic (5.7 GiB) and sandbox (630.7 MiB). The safeguard remained active on the successful commands. No peer target or shared compiler cache was cleaned. Durable distributions, digest, proof and decisions are preserved above. The existing grpc-proto worktree remains locked with Owner requested retained gRPC prototype; deferred migration tasks stay open.

## Historical Integration Diagnostic

The first execution on the previous checkpoint stopped during Schematic initialization after 3.05 seconds with:

    caller may not invoke operation host.publish without a host assignment

The structured Permanent failure preserved plugin origin and the lifecycle/table/binding diagnostic chain. Initialization used an empty assignment list, so later assertions were never reached. An earlier shortened exact filter selected zero tests and was not proof. The owner explicitly classified the missing assignment as an integration defect; it is not counted as a gate failure. The corrected structured assignment boundary and successful Gate 1 supersede that stop decision.

## Original Validation and Retention

The individual ordered proofs passed Gates 1–4. The optimized Gate 5 measurement completed and failed on its own numbers; Gate 6 remains unmeasured. The final focused rerun on the completed source passed all four stories sequentially: four passed, zero failed, 30.63 seconds of test execution. Its repeated cancellation settlement was 0.478–0.774 ms. Child private-memory growth was 471,040 / 5,730,304 bytes and parent growth was 344,064 / 2,138,112 bytes for the smaller/larger payloads; the original fixed envelope remained unchanged. No full simulation or full test lane ran.

mise run check:rust passed: Rust formatting; workspace Clippy; Schematic native Clippy; Schematic Wasm guest Clippy; sandbox Wasm guest Clippy; and Cargo Machete with no unused dependencies. The opt-in test configuration and optimized measurement binary passed their own matching Clippy selections through the focused task. mise run format:python --check passed for the one-line task CLI repair. The full staged candidate diff against origin/dev passed the owner's required case-insensitive term scan with no matches, and git diff --check was empty. Source checks do not override Gate 5's measured failure.

The source and measurement checkpoint 7eb470c6fc5a228da95b5111ddcacadaaa7ba2c1 was pushed to feat/plugin-grpc-prototype and verified against the remote SHA. Delivery uses normal branch pushes only; no PR or integration merge was created.

After preserving these results and the executable digest, native cargo clean completed through mise's tool environment against all three Cargo-metadata-resolved, non-linked targets inside the owned worktree: root (751.4 MiB), Schematic (5.9 GiB) and sandbox (630.7 MiB). No other worktree or shared compiler cache was cleaned. All prototype child processes had settled. The existing grpc-proto worktree remains retained and locked at the owner's request.

## Owner-Directed SDK Simplification

### Decision and Current Shape

On 2026-11-07 the owner accepted the gRPC results and prioritized smaller SDKs: "i am satisfied with the grpc results, i would try to minimize the machinery needed in the sdks. Above 200 MiB/s is good for now." This new commit restores the b12c8716 transport source instead of rewriting the credits checkpoint. Data is a unary request with a server-streaming response. Both peers use the 64 KiB stream window, 2 MiB connection window, fixed windows with adaptive expansion disabled, 64 KiB maximum HTTP/2 frames and NODELAY. The existing 64 KiB codec allocation, one-message yield threshold, Bytes payload sharing, two-message producer queue, 2 MiB message ceiling and 16-call admission remain. The custom consumption semaphore, receipt reader, phase-13 messages and receiving wrapper are removed. Gate 4's source is restored exactly to b12c8716, including every assertion.

Gate 5 now requires each gRPC median to be strictly above 200 MiB/s. Framed remains in each alternating pair and is reported without deciding acceptance. Payloads are still 65,928 and 1,048,968 Arrow IPC bytes; timed counts are 4,072 and 256, eight warmup batches precede each size, and seven repetitions alternate carrier order. CPU samples include only GATE_5_STARTUP through GATE_5_RESULT; builds are excluded. BELOW_FLOOR and exit 1 report an unmet numerical floor; when timed mean CPU exceeds 80%, the owner classifies it as inconclusive pending the orchestrator's quiet-machine repeat. No processes belonging to other workers are stopped.

### Current Gate 6 and SDK Cost

The restored three prototype files contain 497 lines in kit/prototype.rs, 320 in prototype/client.rs and 165 in prototype/codec.rs: 982 physical Rust lines, down 92 from the credits shape's 1,074. With the approximately 100-line credit allowance removed, Gate 6 returns to the 1,900–2,900 replacement-Rust-line planning range against 2,192 optimistic removable lines: 292 net lines removed at best, through 708 net lines added. Generated code, tests, manifests, docs and new language SDKs are excluded on both sides. This remains an estimate; it does not establish net removal or complete a migration.

C++ and Go can consume Data with an ordinary generated server-streaming gRPC client and its normal HTTP/2 flow control. No application acknowledgments, receipt queue, credit semaphore, bidirectional request stream, custom HTTP body, pipe adapter or output-copy bypass is required. Each SDK must configure its own gRPC implementation's stream/connection windows, message/frame limits, adaptive-window behavior, NODELAY and admission as available, then remeasure its buffering; Rust's Gate 4 does not prove another runtime's bound. Bytes is Rust's storage representation; the wire payload is an ordinary Protobuf bytes field.

Beyond generated stubs and those transport settings, reproducing this prototype requires each language to implement:

- Private child-process bootstrap: endpoint discovery, random bearer credentials, generation identity, authenticated metadata, lifecycle initialization/shutdown and process/connection teardown.
- Protocol admission and dispatch: version and generation validation, operation identity and fingerprint matching, structured initialization assignments, request/response projection and structured failure JSON/status translation.
- Invocation ownership: IDs and context, prepare/initialize/attempt/finalize/cancel sequencing, completion status, cancellation on abandonment, retirement fences and settlement. Separate control and data connections preserve control progress under unread data.
- Arrow IPC production/decoding and ordered batch/invocation/count accounting; the server needs a bounded producer queue with cancellation while waiting for capacity and EOF/error cleanup. Consumers own retained batches after delivery. Completion/cancellation/progress remain control RPCs; per-batch consumption sends nothing.
- Equivalent lifecycle, cancellation, stale-generation and slow-consumer conformance cases. The prototype's blocked/produced counters are measurement instrumentation, not additional client flow control.

A complete production SDK would additionally need the still-unimplemented host callbacks/publications, typed upload/download/exchange streams, observers, retry/context/claims behavior and automatic supervision integration already included in the replacement estimate. Only Rust/Tonic is implemented and tested. Checked-in language-neutral protocol definitions and shared conformance remain deferred under task 3.2; no C++ or Go port is claimed here. README.md and docs/product-guide.md continue to describe the unchanged shipping carrier accurately, so neither needs an edit.

### Linux Availability

Docker reports a Linux daemon and already cached managed Ubuntu images. Inspection of the existing repository harness found that examples/schematic/sidecar/tests/system/grpc_prototype.rs::memory unconditionally executes powershell.exe and reads Get-Process PrivateMemorySize64/WorkingSet64. The cached repository executor ci-ubuntu24:d30423dd1fad09c13c0d655edf07dbeff9c89415ebbc1ba10047523d78f2d9de has Python and mise but neither powershell.exe nor pwsh. There is no Linux memory sampler for the unchanged Gate 4 assertions. A native Linux process-memory implementation preserving those assertions is missing; without it the full Gates 1–5 sequence cannot run. No Linux source adaptation, image build, tool installation or compilation was attempted. Linux has no measured gate or throughput result; task 3.1 remains open for that proof and owner migration direction. Tasks 3.2 and 3.3 were not started.

### Restored Windows Correctness Proof

The exact focused mise test selection (schematic-sidecar, prototype-grpc, system-prototype, grpc_prototype::, --test-threads=1 --nocapture) ran matching Clippy and all four stories: four passed, zero failed, 61.30 seconds. Gate 1 retains real plugin initialization, ListDirectories, Arrow decoding and shutdown. Gate 2 settled eight unread-response cancellations at exactly four produced batches in 0.787–1.176 ms, without later progress, then completed a new transfer. Gate 3 retained structured startup refusal, physical restart, prepared-call fencing, retired credentials, stale-generation refusal and shutdown settlement.

| Arrow Bytes | Child Baseline / Peak / Growth (bytes) | Parent Baseline / Peak / Growth (bytes) | Unchanged Allowance (bytes) |
| --- | ---: | ---: | ---: |
| 65,928 | 3,604,480 / 4,104,192 / 499,712 | 1,748,992 / 2,031,616 / 282,624 | 8,388,608 |
| 1,048,968 | 3,637,248 / 7,016,448 / 3,379,200 | 2,699,264 / 3,944,448 / 1,245,184 | 8,388,608 |

Gate 4 used the unchanged 64/4,096/65,536 requested counts and eight slow checkpoints per transfer. Produced minus consumed never exceeded four. Restoring HTTP/2 flow control passed the original assertions without application credits or any changed test bound. These process-fixture observations do not establish a universal allocator bound.

### Restored Windows Gate 5: Inconclusive Under Load

The unchanged optimized focused mise run completed seven alternating paired repetitions for each payload, with profiling disabled and no concurrent validation from this task. Timings, Arrow verification boundaries, eight-batch warmups and 4,072/256 timed counts are unchanged. All values below are useful Arrow MiB/s.

| Arrow Bytes | Repetition | gRPC (MiB/s) | Framed Stdio (MiB/s) |
| --- | ---: | ---: | ---: |
| 65,928 | 1 | 123.095 | 167.851 |
| 65,928 | 2 | 134.850 | 155.554 |
| 65,928 | 3 | 178.204 | 195.409 |
| 65,928 | 4 | 139.901 | 194.558 |
| 65,928 | 5 | 165.136 | 219.428 |
| 65,928 | 6 | 156.898 | 222.110 |
| 65,928 | 7 | 163.934 | 230.305 |
| 1,048,968 | 1 | 178.708 | 446.267 |
| 1,048,968 | 2 | 226.324 | 439.771 |
| 1,048,968 | 3 | 254.131 | 418.750 |
| 1,048,968 | 4 | 254.414 | 464.452 |
| 1,048,968 | 5 | 230.532 | 420.215 |
| 1,048,968 | 6 | 251.113 | 461.531 |
| 1,048,968 | 7 | 232.246 | 452.829 |

| Arrow Bytes | gRPC Median [Min, Max] (MiB/s) | Framed Median [Min, Max] (MiB/s) | gRPC / Framed | Floor Result |
| --- | ---: | ---: | ---: | --- |
| 65,928 | 156.898 [123.095, 178.204] | 195.409 [155.554, 230.305] | 0.8029 | Inconclusive: CPU Above 80% |
| 1,048,968 | 232.246 [178.708, 254.414] | 446.267 [418.750, 464.452] | 0.5204 | Pass: Median Above 200 |

The 61 timed Windows GetSystemTimes samples span 2026-11-07T08:08:28.869+01:00 through 2026-11-07T08:09:01.870+01:00, with mean 99.78%, minimum 97.75% and maximum 100.00%. Compilation is excluded. The smaller median is below the floor while mean CPU exceeds 80%, so the owner-directed result is inconclusive, not a failed performance gate; the orchestrator must repeat this shape on a quiet machine. The larger median passes. The harness reported BELOW_FLOOR and returned exit 1 to preserve the unmet numerical predicate; this is not a compiler or correctness-test failure. Framed is reported only as the paired comparison. No outlier was removed, other worker stopped or additional tuning attempted.

The measured executable SHA-256 is 6a07f985a765462c564c67f3ffb36edfd7503ea99e29230da80a90a45cecfe05. Optimized compilation used two Cargo jobs because shared commit headroom was approximately 2 GiB near completion; no allocation failure occurred. All owned measurement children settled before the result.

### Simplification Validation and Delivery

mise run check:rust passed: formatting for all three workspaces, workspace Clippy including tests, Schematic native CLI/sidecar Clippy, Schematic Wasm guest Clippy, sandbox Wasm guest Clippy and Cargo Machete without unused dependencies. The opt-in process and optimized measurement selections passed their own matching focused Clippy. The four focused stories passed; the only nonzero measurement exit was the load-inconclusive Gate 5 numerical floor result described above. No compiler allocation failure or correctness/check failure occurred. Strict OpenSpec validation passed and git diff --check was empty. No full lane, CI simulation, PR, merge, migration or Dokime change was started. Linux is unmeasured for the explicit sampler limitation above.
