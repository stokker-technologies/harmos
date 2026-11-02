# Runtime Observer Cadence

## Approved API

```rust
#[harmos::observer(interval_ms = DEFAULT_INTERVAL_MS)] // Required, at least 1 ms.
impl Poller {
    // Optional Activation Override
    fn interval(resources: &ExecutionContext) -> Result<Duration, Error> { /* settings */ }
}
```

The macro sets `Observer::INTERVAL` to the declared default, validates it even
when an override exists, and delegates an optional authored interval method.
The trait's fallback returns `Self::INTERVAL`; manual observers declare the same
constant. Streams remain cadence-less. Static Entry/Declaration and
ObserverDefinition do not advertise a default as effective cadence.
Activation resolves the interval once, rejects zero, and passes the snapshot to
the scheduler for initialization retries and ticks. Error and panic handling
refuse activation without a spinning timer. Resource changes apply on reactivation.

## Paired Consumer

Dokime: ../../dokime.wt/hostsettings, feat/host-configurable-settings.
The owner approved an ignored checkout-local Cargo patch during implementation.
No path-resolved lockfile or override is committed. A verified published upstream
revision and final Dokime pin remain follow-up work under separate authorization.

## Ownership And Acceptance

Implementation Lead owns Harmos source, macros, consumers, tests and docs.
The orchestrator owns this specification, final review, checks and commits.
Research And Review audits consumer coverage read-only.
Inventory includes harmos-domain inventory encoding, harmos-sidecar observer
runtime/attachment, harmos-macros observer declaration, harmos-runtime observer
definitions and sidecar adapters, all native/sidecar tests, cargo-harmos templates,
examples and maintained authoring guides/README claims.

Controlled-clock tests must prove a resource-supplied interval controls emission,
with invalid duration refusal and lifecycle behavior preserved.
Use mise focused tests first; final proof is requested workspace check and format.
No release build, full lane, CI simulation, push or publication is authorized.
