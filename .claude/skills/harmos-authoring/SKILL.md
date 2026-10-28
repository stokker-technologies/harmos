---
name: harmos-authoring
description: Implement consumer Harmos operations, runtime resources, storage integration, composed plugins and transport exposure.
---
<!-- Rendered by stkr project update from .agents/skills/harmos-authoring/SKILL.md; sha256:a0100bd7386e5648d0241f9d4f4f277da715dd3746a0928f2ebe92678035926b -->

# Harmos Authoring

Use this skill when implementing an application that consumes Harmos, including
shipped example consumers. It is self-contained; no global company skills or
repository-specific build system are required.

## Ownership Before Wiring

Keep shared models, records, streams and product settings in the domain.
Use one vertical file per operation: a complete Definition Module skeleton followed by a complete runtime-gated Runtime Module
skeleton, each with its own Tests section. The operation struct is its request message. A reusable operations crate exposes
contracts by default and enables execution with `runtime`; resource dependencies
can be optional. A self-contained plugin uses the same layout under `operations/`.

One catalog provides `catalog()` for portable definitions and an executable
`CATALOG` descriptor through `operations::OperationCatalog`. Applications and
plugins share `Lifecycle`: acquire resources in async `prepare(config, setup)`,
register each once with `setup.resource::<T>(value)`, and retain their owners for
stop and cleanup. Application setup places catalogs with
`setup.operations::<Catalog>(lane)`; plugin `const CATALOGS` supplies its complete
inert and executable inventory. `Runtime::launch` validates setup before
initialization; `launch_with` replaces prepared resources by type for tests.
`Context::resource` reads declared resources from the frozen owner context.
Native, sidecar and guest bodies implement the common SDK `Operation` trait;
carrier composition supplies target-compatible resources. Keep one implementation
where dependencies support both carriers. Do not add forwarding operations.

## Choose The Relevant Reference

- [Operations](references/operations.md): messages, metadata, catalogs and handlers.
- [Runtime](references/runtime.md): configuration, resources, state and lifecycle.
- [Storage](references/storage.md): admission, retained ownership, queries and files.
- [Plugins](references/plugins.md): composed native plugins, requirements and guests.
- [Transport](references/transport.md): gRPC exposure, codecs and generated contracts.

The architecture skill owns the detailed consumer workspace convention. These
references explain implementation without assuming the caller has loaded it.
Verify concrete APIs against the selected dependency version before copying
an example; code fragments here illustrate the named boundary, not complete
standalone applications.
