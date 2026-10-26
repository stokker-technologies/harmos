---
name: harmos-authoring
description: Implement consumer Harmos operations, runtime resources, storage integration, composed plugins and transport exposure.
---

# Harmos Authoring

Use this skill when implementing an application that consumes Harmos, including
shipped example consumers. It is self-contained; no global company skills or
repository-specific build system are required.

## Ownership Before Wiring

For a reusable product, put portable operation declarations, models, records,
streams, prompts and product settings in its domain crate. Implement operations
and retain live resources in its runtime crate. Interfaces consume the product
entry library, enabling its optional runtime feature when hosting execution.

A self-contained crate can colocate each declaration and handler in a vertical
`operations/` module. Plugin-private operations use the same approach. Do not
create duplicate product declarations or forwarding operations to expose the
same behavior through another interface.

Declaration catalogs describe contracts; handler catalogs assemble executable
implementations. Plugin advertisement and runtime binding are distinct steps.
Use the installed Harmos version's carrier and context APIs: a handler authored
for one concrete context is not automatically reusable in every carrier.

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
