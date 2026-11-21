---
sidebar: { order: 1 }
---

# Rust SDK

The `harmos` crate supplies reusable connection plumbing and native or WebAssembly plugin carrier helpers. Products own their authoring SDKs and generated client roots.
Import shared values from `harmos`, client facilities from `harmos::client`, and plugin
facilities from `harmos::plugin`. Wire modules remain private implementation details.

## Plugin Context

```rust
use harmos::plugin::{Context, HostOperation, Plugin, Registrar};

fn register(ctx: &mut Registrar<'_>) -> harmos::Result<()> {
    ctx.register(HostOperation::PluginsSettingsList)?;
    ctx.register(HostOperation::TimeGet)?;
    Ok(())
}

let plugin = Plugin::new("example", "1.0.0").register(register)?;

async fn observe(context: &Context) -> harmos::Result<()> {
    let settings = context.settings().list().await?;
    let now = context.now().await?;
    Ok(())
}
```

`context.settings().list()` and `context.host().plugins.settings.list()` invoke the
plugin carrier's own-settings facility. Its result includes effective values,
revision, declarations and cut. The host defaults a missing selector to the trusted
calling plugin and refuses another plugin's name. Settings Set is absent from the
plugin surface. Time, discovery and own invocation lookup, cancellation and watching use plugin
facilities. Host operations have no wire services; product wrappers own client actions.

Client generation selects operations callable by clients; context generation selects
operations callable by plugins. Descriptor generation also emits `plugin.rs` with
`PluginContext::new(context)` and its product-specific `host` root. Both generated
surfaces read the declared audience. The host enforces that declaration independently
of generated methods; runtime assembly has no audience overrides.

## Application and Dynamic Calls

Application typed calls belong to the application's generated SDK; see the
[Schematic walkthrough](../../examples/schematic/cli/src/walkthrough.rs). The reference
client example uses the fixture product's generated wrapper root.

[The separate dynamic example](../../tools/conformance/examples/dynamic.rs) discovers
`conformance.step.0`, an echo operation using `Value` from the
[shared conformance fixture](../../sdk/tests/fixtures/conformance/proto/conformance/model/v1/fixture.proto).
It passes an encoded payload and the discovered fingerprint to
`client.invocations.create(operation, fingerprint, payload)`. Use this path when
choosing a contract at runtime rather than binding generated application messages.

## Validation

Select the `harmos` and `harmos-sdk-derive` packages through the repository's build
and test tasks. The SDK supports native plugin carriers and the wasm target with
the `wasm` feature. Integration stories preserve child-call correlation, controlled
host time, settings, streaming and cancellation. Hosted CI and Nightly provide full
platform validation.
