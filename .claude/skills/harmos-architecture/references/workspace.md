<!-- Rendered by stkr project update from .agents/skills/harmos-architecture/references/workspace.md; sha256:d3c99f459dd1ab6c66c0d5f036c7888d702e1c98ddb912181e6fd4e5be61043b -->

# Consumer Workspace

This is a recommended consumer layout, not a claim that an existing example
already has these exact directories. Product crates and a single sidecar or guest live under `crates/`. Several plugins use
`plugins/sidecars/<name>` and `plugins/guests/<name>`, each with `publish = false`. Keep only populated crates and modules.

Include the populated plugin paths as Cargo workspace members alongside
`crates/*`, for example `members = ["crates/*", "plugins/sidecars/*", "plugins/guests/*"]`.
Omit a plugin glob when that directory has no crates.

```text
product/
├── Cargo.toml
└── crates/
    ├── product/
    │   └── src/
    │       ├── lib.rs
    │       └── prelude.rs
    ├── product-domain/
    │   └── src/
    │       ├── lib.rs
    │       ├── error.rs
    │       ├── config.rs
    │       ├── model/
    │       │   └── instrument/
    │       │       ├── mod.rs
    │       │       └── config.rs
    │       ├── transactions/
    │       ├── state/
    │       ├── records/
    │       └── streams/
    ├── product-operations/
    │   └── src/
    │       ├── lib.rs
    │       └── instrument/
    │           ├── mod.rs
    │           └── read.rs
    ├── product-runtime/
    │   └── src/
    │       ├── lib.rs
    │       ├── error.rs
    │       ├── runtime.rs
    │       ├── instrument/
    │       ├── transactions/
    │       └── state/
    ├── product-automation/
    ├── product-grpc/
    ├── product-cli/
    ├── product-gui/
    ├── product-sidecar/
    ├── product-guest/
    └── product-plugin/
        └── src/
            ├── lib.rs
            ├── main.rs
            ├── config.rs
            ├── error.rs
            └── operations/
                ├── mod.rs
                └── inspect_session/
                    ├── mod.rs
                    ├── windows.rs
                    └── unix.rs
```

The operations crate's `read.rs` owns request, reply, declaration metadata and
the runtime-gated implementation. Shared values belong in domain models.
Transaction, record and stream contracts retain their domain owners; execution
composition and resource construction remain in the runtime. Interfaces enable
the operations crate's `runtime` feature only when they execute its factories.

`runtime.rs` is an illustrative cohesive assembly module, not a mandatory
filename. Use one module per operation, with complete definition and runtime skeletons. Resource modules need not mirror the domain
tree. `product-automation` and `product-grpc` illustrate independent external
clients, not required crates.

## Entry Library

Illustrative manifest sections for the product facade:

```toml
[features]
default = []
runtime = ["dep:product-runtime"]

[dependencies]
product-domain = { path = "../product-domain" }
product-operations = { path = "../product-operations", default-features = false }
product-runtime = { path = "../product-runtime", optional = true }
```

```rust
// product/src/lib.rs
pub use product_domain as domain;
pub use product_operations as operations;

#[cfg(feature = "runtime")]
pub use product_runtime as runtime;

pub mod prelude;
```

The prelude re-exports selected public domain types and, behind the same
feature, selected runtime types. Do not wildcard-export all implementation
modules. A pure facade does not need its own error type.

```toml
# An Interface That Only Uses Contracts
product = { path = "../product" }

# An Alternative Interface That Hosts Execution
product = { path = "../product", features = ["runtime"] }
```

These are alternative dependency declarations. Published consumers select the
appropriate version; publishable internal dependencies also need valid version
requirements. The product's optional `runtime` feature is a consumer convention,
also the feature name on Harmos's facade. The operations crate keeps resource
dependencies optional under the same feature.

## Self-Contained And Composed Plugins

A small standalone plugin can have just `lib.rs`, its entry point, resources
and `operations/inspect/mod.rs`. That module contains its one declaration and
reply, with adjacent native implementation files. A domain crate is warranted when those contracts need
independent consumers, not simply because the crate uses Harmos.

A composed plugin enables `product/runtime`, retains the runtime owner and
exposes the original product contracts. Its private `operations/inspect_session/mod.rs` retains the same definition
and adjacent native implementations. If another interface needs that behavior, move
its vertical operation into the reusable operations crate rather than copying it.

Host-specific files such as a plugin framework's `settings.rs` may remain;
do not rename or merge them solely to match this illustration. Framework suites,
templates and evidence are governed by that framework. Harmos guidance owns
only the common contracts, resources and carrier integration.

## Harmos Built-Ins

Harmos uses the same separation: `harmos-domain` owns shared portable models,
`harmos-operations` owns the 30 vertical built-in operation files and concern
catalogs, and `harmos-runtime` supplies SDK-defined dependency handles and
storage engines. `harmos` exposes them through `builtins`. Execution is gated
by the operations crate's `runtime` feature. Carrier kits use its `lifecycle`
subset for SDK plugin controls without host storage dependencies. Portable
consumers need neither SDK nor storage execution dependencies.
