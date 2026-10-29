# Operations

## Vertical Authoring

One module owns each operation. A plugin uses `operations/<op>/mod.rs`; its
platform implementations are adjacent `windows.rs` / `unix.rs` files with gates
only on the `mod` declarations. A reusable operations crate may use one file
for a platform-independent operation. Its definition division contains messages, the operation struct, policy, contract implementations and
contract tests. The runtime division follows the definition's Tests section and contains its own complete skeleton, including runtime-only
tests. Shared models remain in the product domain; resource libraries are optional execution dependencies.

```rust
/* === --- === Definition Module === --- === */

/* === Modules === */


/* === Re-Exports === */


/* === Imports === */

#[cfg(feature = "runtime")]
use device::Device;

/* === Constants === */

const POLICY: harmos::OperationPolicy = crate::POLICY;

/* === Macros === */


/* === Type Aliases === */


/* === Structs === */

#[harmos::message]
pub struct Reading { pub value: f64 }

#[harmos::operation(id = "temperature.read", response = Reading, policy = POLICY,
    visibility = Shared, resources = [Device])]
pub struct Read { pub channel: u32 }

/* === Enums === */


/* === Traits === */


/* === Implementations === */


/* === Public Functions === */


/* === Private Functions === */


/* === Tests === */


/* === --- === Runtime Module === --- === */

#[cfg(feature = "runtime")]
mod runtime {
    /* === Modules === */


    /* === Re-Exports === */


    /* === Imports === */

    use super::*;
    use harmos::{Context, Operation, Response};

    /* === Constants === */


    /* === Macros === */


    /* === Type Aliases === */


    /* === Structs === */


    /* === Enums === */


    /* === Traits === */


    /* === Implementations === */

    impl Operation for Read {
        type Error = device::Error;

        async fn run(self, ctx: Context<'_, Self>) -> Result<Response<Reading>, Self::Error> {
            let device = ctx.resource::<Device>();
            Ok(Reading { value: device.read(self.channel).await? }.into())
        }
    }

    /* === Public Functions === */


    /* === Private Functions === */


    /* === Tests === */
}
```

The operations crate uses `harmos` with `default-features = false` and
`runtime = ["harmos/runtime", "dep:device"]`. A unit operation struct is a unit request. `response` defaults to `()` and
`visibility` to `Private`; `Shared` and `Host` are short visibility forms. The operation struct is the request message;
its runtime implementation declares only `Error` and consuming `run` (plus optional error classification).

For a platform-only operation, put `#[cfg(windows)]` on its `mod` declaration and its catalog variant. The catalog preserves
variant attributes for both portable projections and runtime registration. Short built-ins may keep their gated implementation inline.

```rust
#[cfg(windows)]
mod desktop;

#[harmos::catalog]
pub enum Operations {
    Read(temperature::read::Read),
    #[cfg(windows)]
    Desktop(desktop::Inspect),
}

impl harmos::Lifecycle for Product {
    type Config = ProductSettings;
    type Error = DeviceError;
    type Context = harmos::Startup<Self>;

    async fn prepare(config: Self::Config, setup: &mut harmos::Setup<Self>) -> Result<Self, Self::Error> {
        // Resources
        let device = std::sync::Arc::new(Device::open(config.device).await?);
        setup.resource::<Device>(device.clone());

        // Lanes
        setup.lane(Lane::Interactive, harmos::operations::LaneSettings { capacity: config.capacity });

        // Operations
        setup.operations::<Operations>(Lane::Interactive);
        Ok(Self { device })
    }

    async fn initialize(&mut self, _: &Self::Context) -> Result<(), Self::Error> { Ok(()) }
    async fn stop(&mut self, _: &Self::Context) -> Result<(), Self::Error> { self.device.close().await }
    async fn cleanup(&mut self) -> Result<(), Self::Error> { Ok(()) }
}

let runtime = harmos::Runtime::<Product>::launch(origin, settings).await?;
let reading = runtime.operation(Read { channel: 3 }).retry(retry).lane(Lane::Interactive).run().await?;
runtime.shutdown().await?;
```

`setup.resource::<T>(value)` accepts owned or shared values and retains one
allocation per concrete type. Independent resources with the same Rust type need
newtypes. Launch collects duplicate resource/catalog, missing-resource and
unknown-lane errors before initialize runs. Tests override prepared allocations
through `Runtime::launch_with(origin, config, |setup| ...)` and use the same
catalog composition. Owner stop/cleanup closes resources after supervised drain.

`ctx.call()` exposes portable call metadata. `ctx.resource::<T>()` returns `Arc<T>` infallibly for a type listed in `resources`.
`operations = [other::Read]` permits `ctx.operation(other::Read { channel: 3 }).await`; catalogs aggregate these callees into
plugin requirements. Explicit grants still belong to the plugin. There are three verification layers:

- Compile: generated `Requires<T>` and `Invokes<T>` bounds reject undeclared resource and operation access.
- Activation: the complete registration requires all resources and callee contracts. Cross-plugin calls require `Shared`, or `Host`
  for a host caller; missing targets and incompatible fingerprints are refused before execution.
- Run: operation errors, deadlines and carrier faults remain fallible.

`input = Item` and `output = Item` derive `StreamEndpoints`: `()` for unary calls, `StreamInput<Item>`, `StreamOutput<Item>`,
or their pair. Borrow endpoints with `ctx.streams() -> &Self::StreamEndpoints`; input receives serialize through an asynchronous mutex.
Invocation builders accept `.endpoints(...)` and remain inert until `.start()` or `.run()`.

Owners (plugin, artifact, sidecar record and stream owners) are one `[a-z][a-z0-9_]*` segment, such as `obc_plugin`.
Dots and hyphens are refused in owners, including by `#[plugin]` at compile time. Reserved identity rules still apply.
Local operation IDs use dotted segments `segment(.segment)*`, each with that same grammar. The runtime prepends the owner once:
`temperature.read` becomes `plugin_name.temperature.read`. Built-ins include `host.operations.cancel`, `harmos.initialize`
and `storage.append_file`. `codec = manual` preserves an explicitly authored message codec.

`Message` supplies encoding and decoding; `Portable` supplies structural fingerprints and payload digests. Retries run a clone
of the original operation request. Large payloads belong in streams. The low-level `Carrier` lifecycle is used for carrier proxies
and lifecycle controls; services retain their separate `Service` lifecycle.
