# Harmos

**A Rust runtime for applications that need to explain how their state got
here.**

Harmos keeps application state as a deterministic fold over one ordered,
append-only journal:

```text
state = origin + committed history
```

That journal is the source of truth. Recovery replays the same changes that
produced the state the first time. Undo records an inverse. Historical reads,
persistence, projections, and audit consumers all follow the same order
without becoming a second authority.

`runtime.state` is the application's `StateManager`: it receives transactions
and coordinates undo/redo, history, and recovery. `runtime.state.state()` returns
a read-only `ManagedState` handle to the same live allocation. The application's
own `State` remains plain data; cloning either handle never copies that data.

[Browse the Examples](examples/) ·
[Open an Issue](https://github.com/stokker-technologies/harmos/issues)

## Why Use It

Stateful desktop tools and instrument hosts often grow the same collection of
hard problems: mutation rules, undo, autosave, crash recovery, background work,
live measurements, plugins, and an audit trail. Implementing each one against a
different view of state makes their disagreements the application's problem.

Harmos gives them one position space and one commit order.

- **Checked transitions.** A single writer validates and commits typed changes
  atomically.
- **Replay and recovery.** Rebuild state from the journal, with snapshots as an
  optimization rather than another truth.
- **Undo, redo, and history.** Read or reverse changes by durable identity.
- **Persistence ports.** Fold the committed order into the storage engine and
  file format your application owns.
- **Typed streams.** Carry high-rate measurements outside the journal while
  preserving state context and exact loss accounting.
- **Supervised work.** Run finite operations and resident services with explicit
  lifecycle rules.
- **Native and Wasm artifacts.** Load directly declared sidecars and guests
  behind bounded execution contracts.
- **Deterministic simulation.** Exercise application workflows without real
  time or external equipment.

## When It Fits

Harmos fits when the sequence of accepted changes matters as much as the latest
value. Editors, engineering tools, instrument hosts, and configuration systems
benefit when recovery, audit, undo, and observation must agree.

It is not a database, a message broker, or a high-rate sample store. If an
external database already owns the authoritative state, keep it authoritative.
If data has no effect on replay, put it in a derived view, a file, or a Harmos
stream instead of the journal.

## Start From a Pinned Revision

Harmos is pre-1.0 and is not published to crates.io yet. Depend on a Git
revision you have tested:

```toml
[dependencies]
harmos = {
  git = "https://github.com/stokker-technologies/harmos",
  rev = "<tested-commit>",
}
```

An application imports only `harmos`. Optional facilities are Cargo features
that open modules of the same name:

```toml
harmos = {
  git = "https://github.com/stokker-technologies/harmos",
  rev = "<tested-commit>",
  features = ["sidecar-host", "sim"],
}
```

| Feature | What It Adds |
| --- | --- |
| Base crate | Managed state, transactions, history, streams, work, and evidence |
| `artifact` | Declaration contract shared by artifact carriers |
| `sidecar` | Native-process authoring SDK |
| `sidecar-host` | Native-process supervision and runtime bindings |
| `guest` | Portable Wasm guest authoring kit |
| `guest-host` | Wasmtime execution and runtime bindings |
| `grpc` | Operation protobuf, runtime serving and reflection |
| `grpc-tls` | Optional TLS for the runtime listener |
| `componentize` | Component encoder without the host runtime |
| `signal` | Measurement kernels over `&[f64]` |
| `plugin` | Packaging and installation for guests and sidecars |
| `sim` | Seeded deterministic execution |

The complete first application lives in
[`examples/minischematic`](examples/minischematic/). It models state, declares
transactions, commits changes, persists them, loads an artifact, and runs under
simulation. The guide embeds code from that example, so the tutorial and the
compiled program stay together.

## Operations Define The Protocol

Put shared requests, responses and operation catalogs in a small product
`contract` crate depending on `harmos-contract`. Bind live devices and other
nonserializable resources in application or plugin implementations. Add
`grpc(package = "...", service = "...", method = "...")` to an operation to
expose it through `runtime.grpc`; normal registration supplies the route.
Empty startup is valid, and ready methods are available through local inspection
and optional standard reflection.

Application build and plugin packaging export protobuf from those declarations
under Cargo's `target/grpc/<name>/`. Consumers need no separate protocol crate,
RPC implementation tree, or hand-maintained proto files. The default portable
payload codec is Postcard; protobuf remains explicit.

See the [compiled examples](examples/) for crate layout, resource ownership,
streaming calls, and schema export.

## Storage Stays Yours

Harmos defines the contracts for committed history, snapshots, streams, and
evidence, but it does not choose a database or on-disk layout. That direction
of dependency is deliberate: a storage adapter depends on Harmos; the runtime
does not depend on the adapter.

[`examples/jsonl-adapter`](examples/jsonl-adapter/) is a complete worked
implementation with a JSONL journal, snapshots, stream capture, crash recovery,
and an evidence directory. Copy the parts you need and own the resulting
format. It is an example, not a hidden persistence product.

## Plugin Publication Time

Plugin stream queries, observers, and capture batches retain a `PublicationRow`:
Postcard `payload` plus optional `published_unix_micros`. Sidecars stamp submission
before transport queueing. The host preserves that value through batching and
reload; it does not replace an absent clock with receipt time. Guest and direct
publishers may omit it. Device observation time belongs in the authored payload.
Cursors and sequence numbers determine order and gaps, independently of clock
skew or backward clock adjustments.

## Artifacts Cross Explicit Boundaries

Sidecars and Wasm guests are parallel artifact types. Both carry their own
identity, semantic version, and declaration, which the host reads before it
runs any artifact code.

| Artifact | Installed Unit | Boundary | Transport |
| --- | --- | --- | --- |
| Sidecar | Native executable | Supervised process | Binary data with independent control |
| Guest | Wasm component | Bounded Wasmtime store | Component Model imports and exports |

There is no extra manifest or package object between the file and its
declaration. See the
[standalone sidecar](examples/standalone-sidecar/) for the smallest native
artifact and [Minischematic](examples/minischematic/) for a Wasm guest.

## Deliver Records to Listeners

Declare a portable listener with `#[harmos::listener]`, selecting its source and
callback operation. A unified plugin includes the descriptor through
`listeners = Audience`. The host's record and stream owners drive delivery;
the callback uses the ordinary operation lifecycle and retained plugin generation.
The declaration itself performs no resource acquisition or delivery.

## Work On Harmos

The repository supports Windows and Linux. Development tools and commands are
pinned with [`mise`](https://mise.jdx.dev):

```sh
git clone https://github.com/stokker-technologies/harmos.git
cd harmos
mise trust
mise install --locked
mise run sync
mise run test
```

Useful entry points:

```sh
mise run run:minischematic  # run the complete example
mise run check              # formatting, lint, targets, and boundaries
mise run build:release      # exact platform release package
mise run test               # workspace test suite
mise run bench              # recorded Rust and Python benchmarks
```

## Native CI And Release Artifacts

Feature pushes run the complete read-only `check` aggregate on Linux and Windows.
Pull requests into `dev` require approval of the `full-validation` environment before
checks, matching release builds, and covered tests run. Strict required status checks
require the source branch to include the current target. CI installs verified current
stable mise; other tools and dependencies remain pinned and locked. Public required
CI uses no stkr or private credentials. Native jobs use `ubuntu-24.04` and
`windows-2022`; the Windows bootstrap initializes Visual Studio C++ and the SDK.

`mise run build:release` stages `dist/<platform>/` and `dist/harmos-<platform>.zip`.
Downloads contain `LICENSE-APACHE`, `cli/`, `plugins/`, `reports/`, and, after testing, `tests/`.
Acceptance consumes those exact release assets. Missing assets fail CI without a
rebuild. Test fixtures compile separately and reuse compatible outputs. Test jobs
retain JUnit, Cobertura, and failure diagnostics in the native artifact download.
Locally, `mise run test` prepares release prerequisites;
`STKR_COVERAGE=1 mise run test` selects coverage without changing test selection
or instrumenting release packages.

Integration and promotion reuse original full-validation artifacts by immutable ID.
Set `HARMOS_VALIDATION_RUN_ID` to an explicit Actions run ID, or let native merged
pull-request metadata identify the original run. `mise run packages:reuse` verifies
both platforms passed, source trees and resolved inputs match, and package/evidence
hashes match. Missing, expired, failing, or mismatched artifacts fail without rebuild
or restamping. Original downloads expire after 14 days.

Task pull requests squash into `dev`. Promotion from `dev` to `main` preserves
ancestry and requires both platform reuse jobs through `packages:promote`.
GitHub is authoritative; the optional GitLab mirror workflow forwards published
`dev` and `main` without routine force pushes. GitLab CI is disabled.

## Contributing

Focused fixes, careful API discussions, documentation improvements, and new
examples are welcome. Read [`CONTRIBUTING.md`](CONTRIBUTING.md) before opening a
pull request; Harmos has strict crate and feature boundaries.

## License

Harmos is licensed under the [Apache License 2.0](LICENSE-APACHE) (`Apache-2.0`).
