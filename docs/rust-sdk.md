# Rust SDK

The `harmos` crate supplies library clients and native or WebAssembly plugin authoring.
Import shared values from `harmos`, client facilities from `harmos::client`, and plugin
facilities from `harmos::plugin`. Wire modules remain private implementation details.

## Client Calls

[The library client example](../sdk/rust/examples/client.rs) discovers operations and
reads host time. Pass the host URL as its first argument and provide the consumer-owned
retention budget in `HARMOS_CLIENT_RETAINED_BYTES`.

```rust
let client = Client::connect(address, settings).await?;
let operations = client.operations.list().await?;
let now = client.time.get().await?;
```

Methods take request content directly. A response containing one field returns that
field; responses containing several fields retain their named payload. Each `_with`
method takes operation options whose `context` owns that call's metadata, deadline
and cancellation. Operation policies own default deadlines and retries.

Plugin settings use one host resource. A client selects a plugin entry and replaces
its complete settings map through `client.plugins.settings.set(...)`. The Rust
generator makes the plugin name a required client argument to `list(name)`; a
plugin handler may omit the selector to read its own entry.

## Plugin Context

```rust
use harmos::plugin::{Context, HostOperation, Plugin};

let plugin = Plugin::new("example", "1.0.0")
    .requires_host(HostOperation::PluginsSettingsList)
    .requires_host(HostOperation::TimeGet);

async fn observe(context: &Context) -> harmos::Result<()> {
    let settings = context.settings().list().await?;
    let now = context.now().await?;
    Ok(())
}
```

`context.settings().list()` and `context.host().plugins.settings.list()` invoke the
same settings operation as the client. Its result includes effective values,
revision, declarations and cut. The host defaults a missing selector to the trusted
calling plugin and refuses another plugin's name. Settings Set is absent from the
plugin surface. Time and invocation lookup, cancellation and watching are shared
host operations; invocation creation belongs to clients.

Client generation selects operations callable by clients; context generation selects
operations callable by plugins. Composition generation also emits `plugin.rs` with
`PluginContext::new(context)` and its product-specific `host` root. A composition file
can narrow a library operation's callers. Both generated surfaces read that effective declaration, and widening is
rejected. The host enforces caller restrictions independently of generated methods.

## Application and Dynamic Calls

Application typed calls belong to the application's generated SDK; see the
[Schematic walkthrough](../examples/schematic/cli/src/walkthrough.rs). The library
client example has no application protocol dependency.

[The separate dynamic example](../sdk/rust/examples/dynamic.rs) discovers
`conformance.step.0`, an echo operation using `Value` from the
[shared conformance fixture](../sdk/tests/fixtures/conformance/proto/harmos/conformance/v1/fixture.proto).
It passes an encoded payload and the discovered fingerprint to
`client.invocations.create(operation, fingerprint, payload)`. Use this path when
choosing a contract at runtime rather than binding generated application messages.

## Validation

Select the `harmos` and `harmos-sdk-derive` packages through the repository's build
and test tasks. The SDK supports native plugin carriers and the wasm target with
the `wasm` feature. Integration stories preserve child-call correlation, controlled
host time, settings, streaming and cancellation. Hosted CI and Nightly provide full
platform validation.
