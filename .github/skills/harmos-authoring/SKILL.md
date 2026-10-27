---
name: harmos-authoring
description: Implement consumer Harmos operations, runtime resources, storage integration, composed plugins and transport exposure.
---
<!-- Rendered by stkr project update from .agents/skills/harmos-authoring/SKILL.md; sha256:c9ba374f30be9bdda534e50f6f39f8558196dbc41cec43d0ca2ccf4b94415459 -->

# Harmos Authoring

Use this skill when implementing an application that consumes Harmos, including
shipped example consumers. It is self-contained; no global company skills or
repository-specific build system are required.

## Ownership Before Wiring

Keep shared models, records, streams and product settings in the domain.
Colocate each request, response, declaration struct and runtime-gated `Operation`
implementation in a vertical operations file. A reusable operations crate exposes
contracts by default and enables execution with `runtime`; resource dependencies
can be optional. A self-contained plugin uses the same layout under `operations/`.

One catalog provides `catalog()` for portable definitions and
`registration(Dependencies)` for execution factories. Runtime composition inserts
shared `Arc<T>` dependencies, chooses typed lanes with `RegistrationExt`, and
registers the batch. `OperationContext::dependency` reads that snapshot.
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
