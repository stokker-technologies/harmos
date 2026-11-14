# Harmos Protocol

`harmos.model.v1` contains exchanged values without services; `harmos.host.v1`
contains operations executed on the host; `harmos.plugin.v1` contains operations
executed on a plugin and its connection carriers. Paths follow those packages.
One resource owns one proto file, service, operation sections and wrapper pairs;
nested resources are subfolders. Every service, call and message has a comment.

Each method declares `(harmos.model.v1.callers)` as CLIENTS, PLUGINS or ALL.
`mise run check -- --repository` checks these options alongside Buf format/lint.
Generators project CLIENTS/ALL host operations into clients and PLUGINS/ALL into
plugin contexts, with one declaration for shared operations. Public SDK barrels
hide wire packages. Plugin Instance remains callable only by its connecting host.

Plugins report model Declarations and typed SettingsDeclarations. Host resources
include Operations, Invocations, Time, Plugins, nested plugins.Settings, Schemas,
Streams, access and status resources. Settings.List has one richer response for
both callers; a plugin's optional name is restricted to its own trusted entry.
Invocations.Create replaces Invoke; Instance.Invoke keeps its carrier name.
Storage remains in-process. Products own typed stored-data wrappers; model File,
Upload, FileReference, PageRequest and Receipt remain shared values.

The [identity map and consumer work lists](../docs/architecture.md#protocol-boundary-definition-and-consumer-migration)
record the clean pre-1.0 replacement and remaining hand-written integration work.
Products list composed library packages and caller restrictions in their own
protocol/composition.toml, consumed by runtime assembly and SDK generation;
restrictions may only narrow a library declaration.

Regenerate through `mise run generate -- --sdk rust`, then python, csharp and cpp.
Generated output is never edited by hand. The protocol crate's generated modules
are model, host and plugin; Python wire modules are private beneath _wire.
See [Behaviour](harmos/plugin/v1/behaviour.md) for the numbered conformance contract.
Within retained values, field numbers, enum values and native storage encodings
remain unchanged; no compatibility package or duplicate service is retained.
