# Minischematic

Minischematic is the complete journal-first product example. Run the ordinary
editor walkthrough with:

```console
mise run run:minischematic
```

It covers state modelling, transactions, records, streams, persistence,
historical reads, and deterministic simulation.

The CLI also accepts `serve <directory> [--json]`. It opens persistent document
storage through the application's normal launch path and waits for Ctrl-C (or
SIGTERM on Unix). Signal handlers are installed before the `ready` event;
`stopped` is emitted only after runtime settlement. JSON mode emits one object
per line with an `event` field. This command preserves its directory between
runs; it does not execute the walkthrough. Add `--exercise-shutdown` to keep an
observer active and hold one running operation plus one queued operation on the
serial analysis lane. An embedded plugin owns a typed stream listener with one
callback in flight and three pending rows in a four-row window. Readiness confirms both operation states. After a signal,
the command checks that both operations were cancelled, each constructed object
stopped and finalized exactly once, the queued operation never ran, and the
observer settled. It verifies that runtime shutdown finalized the original
listener callback exactly once, then explicitly retires its plugin generation.
It also commits 64 symbol placements with the JSONL sync batch set to 128,
snapshots the accepted state, and reopens the same document
through JSONL recovery after settlement. The `recovered` event confirms exact
state and journal-position equality. Existing symbols are preserved and new IDs
follow the highest existing ID. Exercise operations have a five-minute failure deadline;
acceptance independently enforces a five-second process watchdog and a
one-second target covering settlement and recovery. The listener plugin runs in
the host process; the optional native worker adds child-process settlement.
The exercise checks that `accepted_position` exceeds `stored_position` at signal
receipt by at most 64 entries, then proves shutdown flushed that backlog through
exact recovery. Ordinary launches retain the default of syncing every journal
entry; applications can set `Config::journal_sync_entries` to choose the adapter's
bounded crash-risk window.

Exercise JSON output includes `shutdown-timings` between `stopped` and
`recovered`. Its `operation_stop_ns` entries identify the running and queued
objects, `observer_stop_ns` records the backend observing cancellation, and
`settlement_ns` ends when the original runtime stop returns. `pending_entries`
records the accepted but unstored entry count at signal receipt. All intervals begin
when the CLI resumes from its signal wait, excluding process launch and build
preparation. Recovery is outside that runtime interval. Acceptance saves these
samples as JUnit suite properties and checks their order; two signal samples do
not establish p99 responsiveness or a hardware performance guarantee.

The shutdown exercise can include the native worker built with
`cargo build -p minischematic-worker`: pass `--worker <executable>` alongside
`--exercise-shutdown`. The same runtime owns its provider and bounded 256 KiB
payload traffic. Readiness requires a validated exchange; shutdown joins the
child and its traffic before reopening the JSONL state. JSON timings include
`worker_exchanges`, the count of fully validated responses.

## The Five Crates

| Directory | Crate | What it owns |
|---|---|---|
| [`app/`](./app) | `minischematic-app` | what the editor is and how a runtime runs it: `state/`, `transactions/`, `records/`, `streams/`, `app.rs`, the document, the wire format, the storage composition, `services/autoroute.rs`, and the native readback in `analysis/` |
| [`contract/`](./contract) | `minischematic-contract` | the editor's portable plugin contracts: `autoroute/` and `worker/` |
| [`cli/`](./cli) | `minischematic-cli` | the shell: which document, which configuration, the transcript, and rendering an error once |
| [`plugin/`](./plugin) | `minischematic-plugin` | the Autoroute Wasm guest, over the contract crate alone |
| [`worker/`](./worker) | `minischematic-worker` | the native bounded payload worker, over the contract crate alone |

The direction is one way: contract below app below the shell, contract below the plugin,
and nothing depending on the plugin. `minischematic-plugin` compiles
`minischematic-contract` and has never heard of `minischematic-app` — which is only
possible because the contract crate carries `harmos` with every default feature off,
serde, and nothing else, and compiles for `wasm32-unknown-unknown` with no
`cfg(target_arch)` gate anywhere in it. `mise run check:wasm` builds it for that
target on every run, which is what keeps the claim true.

The domain model is never the contract. `minischematic_contract::autoroute::RenameNet` is
what an autorouter *sends*; `minischematic_app::transactions::net::RenameNet` is
what the editor *does*. The host decodes an invoked payload through the catalog
registered for `net.rename`, so the two shapes have to agree on the wire today —
and [`app/src/services/autoroute.rs`](./app/src/services/autoroute.rs) is both
where that is asserted and where a future divergence would be absorbed.

Only `streams(streams::progress::Progress)` grants guest publications. The
editor's linked `NetVoltage` contract grants nothing to this guest. Its
portable echo operation is declared in `contract/src/autoroute/mod.rs` and
bound by the initialized handler in `plugin/src/operations/echo.rs`. Its selected
catalog and live handler table share the canonical `Operation`/`Handle` contract.

## Autoroute Guest

The same product can load Autoroute as a directly installed WebAssembly
component:

```console
mise run run:minischematic:guest
```

The task builds [`plugin/`](./plugin), converts the Rust core Wasm module into
`target/artifacts/autoroute.wasm`, and passes that component directly to the
application. There is no archive, manifest, contracts crate, or
artifact-specific xtask.

The host sequence is split across the two crates that own its halves:

- [`app/src/services/autoroute.rs`](./app/src/services/autoroute.rs) is the host
  half of the edge. It names the guest and opens the one capability it is
  admitted against — `invoke:net.rename`, and nothing else. Which transaction a
  guest may invoke is the application's decision, not a shell's.
- [`cli/src/guests/autoroute.rs`](./cli/src/guests/autoroute.rs) drives the
  walkthrough over it:

1. Read and validate the guest declaration without compiling or instantiating
   Wasm.
2. Load against the declared `invoke:net.rename` edge.
3. Instantiate with a fuel budget and activate the guest.
4. Observe declared progress publications at 10 and 100 percent.
5. Stop the guest and reopen the journal without it.

The accepted rename remains ordinary product history under principal
`guest:autoroute`; recovery never executes or requires the guest.
