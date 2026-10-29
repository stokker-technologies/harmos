# Runtime Composition

The product runtime is the execution composition layer. It depends on portable
domain contracts and resource libraries; the domain never depends on it.
The entry library exposes both, with runtime opt-in. Interfaces invoke this
composition through the entry library.

## Configuration And Resources

Keep portable settings definitions and pure validation in the domain. Compose
subsystem settings in `config.rs`; keep subsystem-specific settings near their
domain owner. Validate before constructing dependent resources. An interface
may decode CLI arguments or framework settings into those product settings.

Runtime owns live devices, network clients, file handles and worker lifetimes.
Separate settings values from these resources. Construct shared resources once,
retain their owners and give handlers the supported shared access. Do not
reconnect a device for every operation unless that is the device's intended
lifecycle.

Place a substantial external-system boundary in a cohesive resource library,
such as automation or a separate vendor protocol client. Small resources can
remain runtime modules. Keep connection, adapter and collector implementations
beside `operations/`; reserve operation files for callable behavior and its
close helpers.

Consumers supply storage budgets, admission limits, concurrency and response
batch sizes. Harmos supplies protocol constants and correctness constraints.
Do not invent universal operational defaults in reusable guidance.

## Startup And Shutdown

Implement the shared `harmos::Lifecycle` alongside `Application`. Its `Config`
is deserializable and defaultable; `Context` is `Startup<Self>`. Async
`prepare(config, &mut Setup<Self>)` acquires resources, configures settings and
places operations. Group preparation into Resources, Lanes and Operations.
`setup.resource::<T>(arc)` retains one allocation per concrete type; use distinct
newtypes for independent values with the same Rust type. All `Application::Lanes`
are declared automatically; `setup.lane(lane, LaneSettings { capacity })` adjusts
one declared lane.

Call `Runtime::<App>::launch(origin, config).await` to prepare, validate and
initialize the retained owner. Configuration errors are collected before
`initialize`: duplicate resource types, duplicate catalogs, missing declared
resources and unknown lanes remain typed. Tests use the same path through
`launch_with(origin, config, |setup| ...)`; the callback replaces prepared
resources by type and does not replace catalogs. There is no public builder or
consumer resource-store object.

`launch_with` runs prepare before applying overrides. Configure acquisition for
the test environment; the callback changes execution lookups, and the lifecycle
owner still closes the resources it acquired.

`initialize(&Startup<Self>)` runs after validation. `runtime.shutdown().await`
drains operations and residents, then calls the retained owner's `stop` and
`cleanup`. Stop performs domain shutdown; cleanup closes acquired devices,
connections and workers, including when validation prevents initialize. Failed
cleanup retains its owner for retry. Cleanup is attempted when stop fails. If
prepare fails before returning an owner, prepare itself must unwind partial
acquisitions. Setup errors and a prepare error are reported together.

Configure optional subsystem settings through `setup.settings()`. For an
existing storage manager use `setup.settings().storage(&manager)` during prepare;
managed replay, capture and snapshot policy belongs to the same setup.
Simulation uses `Runtime::simulate(origin, config, seed)` or `simulate_with` and
reruns lifecycle preparation with cloned configuration on reboot.

An interface binding handlers must use the runtime's initialized resources,
not construct a second runtime for the same product. An embedded runtime does
not imply a second operating-system process. Plugin lifecycle details and
carrier constraints are in [Plugins](plugins.md).

## State And Persistence

Portable state and transaction definitions belong in the domain. The runtime
assembles managed state, transaction execution, storage and replay using those
definitions. Harmos's state manager owns live state and managed history; the
configured storage backend supplies journal and snapshot persistence.

A transaction is reversible exactly when it implements `Invertible<S>`. Without
that implementation the catalog treats it as an undo barrier. The optional
`inverse = Self` or `inverse = Other` attribute checks the declared pair;
`Invertible<S>::Inverse` must match that declaration:

```rust
#[harmos::transaction(id = "ledger.deposit", inverse = Withdraw)]
pub struct Deposit { pub amount: i64 }

impl Invertible<Ledger> for Deposit {
    type Inverse = Withdraw;

    fn inverse(&self, _before: &Ledger) -> Self::Inverse {
        Withdraw { amount: self.amount }
    }
}
```

Keep evidence cursors distinct from state journal positions. Do not invent
a parallel persistence or undo mechanism simply to expose another interface.
Choose durability according to the backend's documented contract and verify
recovery at that boundary; an in-memory success does not prove restart recovery.

Use `error.rs` and `thiserror` for runtime-owned failures, preserving their
causes. A pure entry facade can re-export those errors without adding a wrapper.
