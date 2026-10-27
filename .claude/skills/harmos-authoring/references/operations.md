<!-- Rendered by stkr project update from .agents/skills/harmos-authoring/references/operations.md; sha256:d18e275a911aad6c81f6d8a4fc46a3ce9bc34da1efc531dbc5340b2b8967e46d -->

# Operations

## Vertical Authoring

One file owns each operation. Its definition division contains messages, the operation struct, policy, contract implementations and
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

let mut dependencies = harmos::Dependencies::new();
dependencies.insert(device)?;
use harmos::RegistrationExt;
runtime.register(Operations::registration(dependencies).lane(Lane::Interactive)).await?;
let reading = runtime.operation(Read { channel: 3 }).retry(retry).lane(Lane::Interactive).run().await?;
```

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

IDs use dotted segments `segment(.segment)*`, each `[a-z][a-z0-9_]*`. The runtime prepends the owner once:
`temperature.read` becomes `plugin_name.temperature.read`. Built-ins include `host.operations.cancel`, `harmos.initialize`
and `storage.append_file`. `codec = manual` preserves an explicitly authored message codec.

`Message` supplies encoding and decoding; `Portable` supplies structural fingerprints and payload digests. Retries run a clone
of the original operation request. Large payloads belong in streams. The low-level `Handler` lifecycle is used for carrier proxies
and lifecycle controls; services retain their separate `Service` lifecycle.
