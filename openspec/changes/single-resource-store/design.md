# Single Resource Store Design

## Locked Lifecycle

```rust
pub trait Lifecycle: Sized + Send + 'static {
    type Config: DeserializeOwned + Default + Send + 'static;
    type Error: std::error::Error + Send + Sync + 'static;
    type Context: Context;
    fn prepare(config: Self::Config, setup: &mut Setup<Self>)
        -> impl Future<Output = Result<Self, Self::Error>> + Send;
    fn initialize(&mut self, context: &Self::Context)
        -> impl Future<Output = Result<(), Self::Error>> + Send;
    fn stop(&mut self, context: &Self::Context)
        -> impl Future<Output = Result<(), Self::Error>> + Send;
    fn cleanup(&mut self) -> impl Future<Output = Result<(), Self::Error>> + Send;
}
```

Async implementations satisfy these Send-future signatures on every carrier. Application
keeps Lanes/State/Transaction/Record/Streams and extends Lifecycle. No resource associated
bundle remains. Prepare owns acquisition; initialize receives Startup for applications or
the plugin carrier context. Runtime retains the original owner through shutdown.

## One Setup Surface

Context selects a configuration role through a hidden associated configuration contract,
so Setup<Self> keeps the locked Lifecycle associated types without a runtime-to-SDK cycle.
Setup resource insertion is shared. Application configuration alone implements lane and
operations registration; plugin Setup has no operations method. Existing runtime settings
must be reachable through Setup without exposing a consuming builder or private batch API.
Required methods borrow mutably and return &mut Self. Runtime::builder becomes internal.
LaneSettings configures a declared lane; all A::Lanes are available automatically.

Setup collects typed duplicate-resource, duplicate-catalog, missing-resource and unknown-lane
errors, reporting all after prepare returns and before initialize or execution. Resource
insertion retains the first allocation on error. One value per concrete type; different
instances require newtypes. Into<Arc<T>> supports owned or shared values; Arc inputs require
an explicit resource::<T> type parameter because Rust otherwise has ambiguous conversions.
Frozen dynamic registration validates against the same store before publication.

## Launch, Tests And Failure Ownership

Runtime::<A>::launch(origin, config) runs prepare, setup validation, runtime assembly and
initialize before serving. launch_with(origin, config, |setup| ...) uses the same path;
its resource registrations replace prepared values by type in a test override mode.
This avoids a second composition path or fake-aware production configuration. Duplicate
resources in production prepare remain errors; overrides only replace values, not catalogs.
Prepare still acquires resources; tests configure safe acquisition and the owner retains cleanup.

Shutdown drains operations/residents, calls stop on the retained owner, then cleanup.
Cleanup is attempted after stop errors; simultaneous failures retain both typed causes.
Cleanup errors remain inspectable after physical retry settles. Partial startup retains application, writer and storage for retry. Failed initialization
retains supervised cleanup and calls owner cleanup. Failed prepare reports setup and prepare
errors together where both exist; no application owner exists to clean up when prepare fails.
There are no per-resource stop callbacks or compatibility APIs. Cleanup closes prepare's
acquisitions even when validation prevents initialize, and retains failures for retry.

Simulation uses Runtime::simulate(origin, config, seed) or simulate_with with a retained
Setup callback. Config is Clone; each reboot repeats prepare, overrides, validation and
initialize against recovered in-memory history, preserving typed StartupError and cleanup
retry ownership; stop/cleanup runs before reboot. Live
attachments remain rejected before initialization. No replayable public builder survives.

## Constant Plugin Catalogs And Facade

```rust
const CATALOGS: &'static [Catalog] = &[Native::CATALOG];
```

A catalog descriptor owns static inert declarations and a hidden execution factory. Portable
contract-only catalog definitions stay free of SDK runtime dependencies. The same CATALOGS
slice drives inert plugin inspection and executable binding; never duplicate it as provides.
Domain inventory reads these descriptors without prepare/initialize. Macros embed their
inert data into used artifact sections. Constant feature selection preserves chosen members.
Declared plugin requirements and streams are never silently feature-gated away. Omitted
requires denotes an empty list; the id-only facade attribute remains valid.

The harmos facade exposes matching sidecar/guest feature, module and attribute namespaces.
Authors use harmos only. Generated paths resolve through harmos or an explicit facade path;
remove caller-ending heuristics. Explicit runtime/sidecar/guest role macros always emit
execution; root portable macros follow the operation library's declared runtime feature.
SDK owner binds observer attachments after Setup resources,
so plugin authors never access a WireTable or bind_operations.

## Inventory And Proof

Integration branch refactor/single-resource-store owns this clean replacement.
Root owns runtime, application tests, integration and delivery.
Implementation Lead owns SDK/shared lifecycle/catalog execution and operations crate.
Research And Review owns macros/facade/domain inventory/packaging inspection. Carrier worker
owns carriers and plugin fixtures. Examples, benches and guidance are explicit integration
items. Writers use disjoint files and report source handoff before source closure.

Focused mise build:rust:focused Clippy/no-run builds cover touched crates/features/examples.
Run operation/resident/shutdown, carrier lifecycle, macro compile and schematic binaries in
parallel. Add aggregate setup errors, application/plugin lifecycle ordering, constant inert
inventory, override/frozen-resource and facade-attribute proofs. No full lanes or ci:simulate.
Strict OpenSpec and stkr project check must pass. Retire with stkr specification finish only
after tracked work closes, commit breaking changes, push, verify SHA and report under 25 lines.
