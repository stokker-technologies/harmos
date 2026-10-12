# Mini Bench

Minibench proves the native artifact path with a simulated device interface.
The product declares an executable into its own assembly; no Wasm guest is
involved.

```console
mise run run:minibench
```

The task builds `minibench-probe-sidecar`, reads its embedded declaration from
the executable bytes without starting it, then boots with it declared as a
resident sidecar. The sidecar initializes with `sim://probe-a`, serves the typed
`probe-bridge.get_sample` operation, publishes a declared `reading`, and exits under the normal
runtime stop sequence.

The CLI also accepts `serve <bridge-executable> [--device <device>] [--json]`.
It installs signal handlers before launching the application, completes one real
probe call, and reports `ready`. Ctrl-C or SIGTERM on Unix triggers the same
`Running::stop()` path as the walkthrough; `stopped` follows successful service
cleanup and child joining. JSON mode emits one lifecycle event per line.

POSIX acceptance starts the host in an isolated process group, sends each signal
to the host alone, and requires the group to be absent after successful exit.
The native child inherits that group, so host exit alone cannot satisfy the
check. A five-second watchdog surrounds a one-second settlement target; native
artifact preparation occurs before timing. Windows Ctrl-C is handled by the CLI,
but the POSIX tests do not establish native Windows signal behavior.

## The Four Crates

| Directory | Crate | What it owns |
|---|---|---|
| [`app/`](./app) | `minibench-app` | what the bench is and how a runtime runs it: [`streams/reading.rs`](./app/src/streams/reading.rs), the entry verbs in [`app.rs`](./app/src/app.rs), the host half of the edge in [`services/probe.rs`](./app/src/services/probe.rs), and the local operations in [`operations/`](./app/src/operations) |
| [`contract/`](./contract) | `minibench-contract` | the probe edge's contract: [`probe/`](./contract/src/probe) holds `Settings`, `ReadSample`, its catalog, `Sample` and `Sampled` |
| [`cli/`](./cli) | `minibench-cli` | the shell: the rig, the subscriber, and the transcript |
| [`probe-sidecar/`](./probe-sidecar) | `minibench-probe-sidecar` | the supervised child process, over the contract crate alone |

The sidecar compiles the contract crate and never the app crate, which is what makes
the contract the one place the two halves of the edge agree on what they exchange:
`Settings` opens an attempt, and `Sample`/`Sampled` are what one call sends and
answers. `minibench-contract` carries `harmos` with every default feature off plus
serde, and `mise run check:wasm` builds it for `wasm32-unknown-unknown` — the
claim a contract crate makes is about what it contains, not about which carrier this
product happened to ship first.

## The Publication Is Declared Twice, On Purpose

`Reading` is **not** in the contract crate. A publication enters a stream, and a
stream row's window and grouping are the host's retention policy rather than a
contract a plugin compiles — so `minibench-app` declares the row it retains and
`minibench-probe-sidecar` declares the row it publishes, exactly as
`examples/standalone-sidecar` does for a bridge with no application beside it.

What the two share is the declared id and schema, named once in
[`contract/src/probe/mod.rs`](./contract/src/probe/mod.rs) as `probe::READING`, and the
agreement is asserted rather than described:

- `Supervised::ingest` refuses a publication whose schema does not match the row it
  decodes into.
- The walkthrough checks the inspected executable's declared
  `(id, schema, window)` against the application's own `Reading` constants,
  before boot.
- [`probe-sidecar/tests/operations/main.rs`](./probe-sidecar/tests/operations/main.rs) checks
  the same triple from the other side.

## Where The Bridge Is Attached

The bridge is declared at build. `Resources::connect` assembles device identity,
calibration and the supervised endpoint. [`app::launch`](./app/src/app.rs) gives
the endpoint to `service_with`, waits for its initialized generation, and binds
its portable operation catalog to the runtime's existing `OperationManager`.
The same initialized resources bind the local report catalog on its serial lane.
Failed readiness or binding awaits runtime cleanup before returning the cause.
If cleanup itself fails, `Error::Cleanup` retains that runtime for an explicit
`stop()` retry along with both causes.

- [`contract/src/probe/mod.rs`](./contract/src/probe/mod.rs) owns `ReadSample`,
  its exact request/response types and finite policy. The initialized probe's
  resource owner prepares the channel once and publishes its reading from the
  canonical handler in [`operations/sample.rs`](./probe-sidecar/src/operations/sample.rs).
- [`app/src/operations/report.rs`](./app/src/operations/report.rs) consumes the
  captured rows and borrows the same initialized calibration through its resource
  owner. The operation's identity is independent of its request fields.
- [`cli/src/commands/run.rs`](./cli/src/commands/run.rs) inspects statically,
  launches, calls typed operations, observes streams, and stops. Its application
  facade exports the contract; the shell does not depend on the sidecar crate.

A native binding retains its original generation and refuses after that
connection closes. A replacement generation needs a fresh explicit binding;
retained invocation state never moves to a restarted child. The deterministic
simulation exercises the bench's stream and lane declaration without binding
native operations or fabricating device responses.

The complete live boundary is gRPC: configuration and registrations cross as
JSON, while operation values, lifecycle messages and samples use protobuf.
The initialized carrier binds one bounded operation table; the host manager owns
retry policy, deadlines and settlement. Publication queues remain bounded and
report sequence gaps rather than blocking device collection.

The walkthrough receives raw readings immediately, then observes one
runtime-owned three-reading statistical group: channels 1–3 have a mean of
3320 mV. The `Reading` declaration marks `millivolts` with `#[harmos(measure)]`;
device and channel are not measured. Statistical groups are independent of gRPC
packing.
