# Go Sidecar Review Slice

Start with [`sdk/go/examples/greet/main.go`](../sdk/go/examples/greet/main.go), then
[`sdk/go/sidecar.go`](../sdk/go/sidecar.go). The example declares one stateless
`go_tools.greet` operation. Its author writes a handler, decodes and encodes the
generated Protobuf messages, observes cancellation and supplies transport budgets.
The SDK owns the generated gRPC service, bootstrap, admission and lifecycle state.
No Dokime code or tooling is involved.

```sh
mise trust
mise install
# PowerShell: $env:CARGO_BUILD_JOBS = '2'
# POSIX: export CARGO_BUILD_JOBS=2
mise run test:rust:go
mise run test:go
mise run check:go
mise run test:rust:grpc
```

The first task builds the Go example, launches it from a Rust host, invokes Greet,
decodes three ordered Arrow IPC batches, cancels an unread response, completes a
fresh transfer, shuts down and checks physical process exit. Its only timeout is
the story's operation policy; correctness assertions do not depend on elapsed
time or OS resource readings. Gates 1–4 select only the existing Rust prototype.
All Rust compilation retains the focused task's preflight and matching Clippy.

## Definitions and Generation

[`crates/harmos-protocol/proto/sidecar.proto`](../crates/harmos-protocol/proto/sidecar.proto)
owns the messages, phase values and two RPC declarations. `harmos-protocol` builds
its Rust messages and service paths directly from that file with Prost. The existing Rust prototype
codec reexports those generated types; its hand-authored message definitions are
gone. Its carrier, buffers, windows and framed path retain their existing behavior.

```sh
mise run prepare:protocol         # Regenerate Checked-In Go Stubs
mise run prepare:protocol --check # Refuse Generated Drift
```

Generation needs only the repo's pinned `protoc`, `protoc-gen-go` and
`protoc-gen-go-grpc`. A Go consumer uses the checked-in stubs with ordinary Go
commands and needs neither Cargo nor the generators. The Go module pins gRPC and
Protobuf in `go.mod`/`go.sum`. Rust generation uses the workspace's `prost-build`.

## Bootstrap and Admission

The parent launches one child with private stdin/stdout pipes and sends one JSON
line `["<64 lowercase hexadecimal token>", <positive generation>]`. The token is
32 random bytes; it never appears in process arguments, environment or output.
The child binds `127.0.0.1:0`, registers the service and writes only `address:port`
plus a newline to stdout. Diagnostics go to stderr. The parent owns abnormal
process termination, including failed bootstrap or lost connections.

Every RPC carries the exact token in `authorization` metadata and the admitted
version/generation in its request Packet. Invalid credentials return
`Unauthenticated`; unsupported version, stale generation or retirement return
`FailedPrecondition`. Control and Data use separate client connections, preserving
control progress while the data receiver is unread. This is authenticated local
cleartext HTTP/2; it is not a remote TLS deployment contract.

Welcome (phase 0) returns the initialization fingerprint in `fingerprint`, shutdown
fingerprint in `context` and exactly one Greet declaration in `declarations`.
For this Go slice, all three fingerprints are SHA-256 of the checked-in proto's
UTF-8 source with LF line endings. Both generators derive that identity; the Rust
story compares it before use. This deliberately conservative whole-schema digest
is a review identity, not Harmos's production per-operation canonical fingerprint
projection. The existing Rust plugin's lifecycle fingerprints remain unchanged.

## Lifecycle and Ordering

Each control invocation carries a nonzero ID. Prepare (1) admits its fully
qualified operation, 32-byte fingerprint, 32-byte context and immutable payload.
Initialize (2) must follow Prepare. Attempt (3) runs once; repeated or out-of-order
attempts fail with `FailedPrecondition`. Finalize (4) releases the invocation after
settlement, or releases an invocation whose attempt never started. Missing IDs
return `NotFound`; an overlapping Prepare returns `ResourceExhausted`.

The reserved `go_tools.harmos.initialize` operation receives `Initialization`:
valid JSON configuration, a 32-byte generation context and no assignments. Host
callback assignments are explicitly refused. Initialization enables Greet and
Data. `go_tools.harmos.shutdown` disables them. Their stateless implementation
owns no application resources. Lifecycle invocations use the same four phases.
Application payloads are the generated `GreetRequest` and `GreetResponse` messages.
An empty Protobuf payload represents the unit lifecycle response.

This slice admits one live control invocation and one live data invocation.
The example supplies message limits, concurrent RPC admission and fixed HTTP/2
windows. It allocates no unbounded request registry or application batch queue.
Handlers own their operation policy and must return on context cancellation.
Operation errors use ordinary gRPC status; full structured Harmos failure
projection, retries, portable context values and claims are follow-up work.

## Arrow Data and Cancellation

Data takes a nonzero invocation ID, one complete pre-encoded Arrow IPC stream in
`payload` and a positive repetition `count`. The Go SDK treats those bytes as
opaque; the Rust story constructs and decodes them with Arrow. Each response
repeats those bytes and echoes the invocation ID, with a zero-based sequence in
`count`. EOF follows exactly the requested number on normal completion. A second
live data request is refused. The host verifies sequence, payload and Arrow rows.

One sender goroutine calls the generated stream's blocking `Send`. Ordinary
HTTP/2 flow control provides backpressure. No application credits, receipts,
per-batch acknowledgment or bidirectional RPC exists. Control Progress (12)
reports the number of sends completed and phase 1 once the sender has settled;
it is diagnostic information, not permission to send another batch. Rust's
WaitBlocked (11) diagnostic is explicitly unsupported by the Go service.

Control Cancel (5) requests control-invocation cancellation without promising
that the operation has returned; its Attempt must settle before Finalize.
CancelData (10) requests cancellation and waits for the sender to exit. The data
handler returns `Canceled` immediately on that signal, which lets gRPC cancel a
blocked Send. This extra sender goroutine is necessary because canceling a Go
application context alone does not interrupt a Send blocked inside gRPC. The
control response waits on the sender's completion, without requiring the client
to read or drop buffered data. Repeated CancelData on the retained flight is
idempotent; progress cannot advance after settlement. Canceling/dropping the data
RPC also stops its sender through gRPC's stream context. Consumers own any bytes
already received; cancellation does not retract them.

## Teardown and Review Limits

Retire (20) fences new requests, cancels owned work, gracefully stops the gRPC
server and waits for its data sender before process exit. The Rust host checks
successful exit rather than killing a normally retired Go child. Its process
handle kills the child on abnormal story exit. There is no automatic restart or
parent-loss watcher in this slice.

The example's data bytes are supplied by the host, rather than authored with a Go
Arrow library. There is no production catalog/registration, callback/publication,
typed upload/exchange, general fingerprint generator or broad conformance suite.
Linux execution, Go throughput, allocation bounds and production supervision are
unproven. The existing framed carrier and all production integration remain in
place. Measurements and exact proof outcomes live in the OpenSpec design.
