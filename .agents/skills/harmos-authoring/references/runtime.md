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

Use a cohesive runtime owner with explicit construction, startup and shutdown
responsibilities. `runtime.rs` is a useful possible module name; `boot.rs`
is not mandatory. The entry point or hosting plugin retains the owner for the
whole execution lifetime.

Make resource acquisition failure-safe: unwind what was successfully acquired
if a later step fails. Ensure partially initialized resources can be released.
On shutdown, stop accepting new application work, settle or cancel active
operations and background producers, then release resources in dependency order.
Keep resources alive until their last supervised consumer has settled.

An interface binding handlers must use the runtime's initialized resources,
not construct a second runtime for the same product. An embedded runtime does
not imply a second operating-system process. Plugin lifecycle details and
carrier constraints are in [Plugins](plugins.md).

## State And Persistence

Portable state and transaction definitions belong in the domain. The runtime
assembles managed state, transaction execution, storage and replay using those
definitions. Harmos's state manager owns live state and managed history; the
configured storage backend supplies journal and snapshot persistence.

Every transaction answers the undo question in its attribute: `inverse = Self`
(the common case), `inverse = Other` for a paired change, or `irreversible`.
A declared inverse is described by `Invertible`, whose `type Inverse` repeats
the declared type:

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
