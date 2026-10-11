# Harmos Examples

The examples are executable architecture checks, not starter templates.

| Example | Command | Boundary Proven |
|---|---|---|
| [Minischematic](./minischematic/README.md) | `mise run run:minischematic` | Journal, persistence, streams, and simulation |
| [Autoroute Guest](./minischematic/README.md#autoroute-guest) | `mise run run:minischematic:guest` | Static Wasm metadata, declared capability edges, bounded initialization, and recovery without the guest |
| [Minibench](./minibench/README.md) | `mise run run:minibench` | Static executable metadata and a supervised native device sidecar over gRPC |

Each application example is a `<name>-app` crate and a `<name>-contract` crate with
its shell and its plugin beside them: the app is what the application is and how
a runtime runs it, compiled for the host only. The contract declares what the host
and its plugins exchange, compiled for the host and for wasm32 alike. That is a crate
boundary rather than a folder because it is the only kind a compiler enforces —
neither half carries a `cfg(target_arch)` gate, and nothing depends on a plugin
crate.

All artifact definitions live with the artifact that implements them. There are
no contracts crates, manifests, archives, or packaging tasks.
