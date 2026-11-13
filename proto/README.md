# Harmos Plugin Protocol

[`harmos/plugin/v1/instance.proto`](harmos/plugin/v1/instance.proto) defines the
host-to-plugin boundary. [`harmos/v1/behaviour.md`](harmos/v1/behaviour.md) holds the
numbered behaviour contract and cross-language conformance fixtures.

The definition pass moves the host APIs into `harmos.client.v1` for client programs
and `harmos.plugin.host.v1` for plugin child calls. `harmos.v1` contains shared
messages and enums only. Folders follow packages; nested resources extend their
parent package, for example `harmos/client/v1/plugins/settings.proto` declares
`harmos.client.v1.plugins.Settings`. Each service owns one file and its own
request/response wrappers. Access is a generated parent of its nested resources.

Shared vocabulary lives in small concept files: positions and read cuts, pages,
receipts, failures, policy, operation contracts, plugin descriptions, invocation
frames, stream declarations/samples, settings, files/uploads, schema descriptions,
transfer defaults and guest frames. There is no aggregate `types.proto` or
`harmos.storage.v1` package. Proto2 transfer defaults preserve their existing values.

`Instance.Get` still reports `harmos.v1.Catalog`: operations, requirements, streams,
product contribution sections, digest and reachable types. Typed setting declarations
use its `SettingsDeclarations` section. This activation description is separate from
registry APIs: clients use typed Operations, Schemas and Streams; plugins use
Operations. No opaque catalog registry switch remains.

Plugin entries own library-stored typed settings. Clients use Plugins.Settings
List/Set; plugins read only their own values through plugin-host Settings.List.
Plugins.Replace replaces a managed bundle in place while preserving entry identity,
settings and product data. Failed staging preserves the previous serving bundle.
Storage operations have no library wire surface: products declare typed wrappers
and call the library in-process. Shared FileReference, File and Upload describe
content without declaring storage calls. Instance.Samples remains the plugin carrier
for demanded interval or device samples.

The [boundary mapping and consumer work lists](../docs/architecture.md#protocol-boundary-definition-and-consumer-migration)
record every former call, generated-module rename, native type ownership and next-pin
change. Hand-written consumers are intentionally unmigrated in this definition pass.
The library Buf module is checked independently; the Schematic worker must replace
its removed storage import and restore its product-owned Buf validation.

Rust bindings live in `crates/harmos-protocol`; the independent reference host and
Rust fixture server live in `tools/conformance`. Regenerate committed protocol and
resource outputs through `mise run generate -- --sdk rust` followed by
`mise run generate -- --sdk python`. The Rust generator also emits the four SDK
client roots. Python client wire modules use `harmos._wire.client` to coexist with
the authored `harmos.client`; protobuf identities remain unchanged by this module rule.

Within a version, never reuse field numbers or enum values for a different meaning;
preserve removed identities when compatibility is promised. This owner-authorized
boundary replacement migrates all consumers together at their next pin. Instance
peers advertise revisions in Get; an incompatible contract needs a new package.
Unknown protobuf fields are ignored. The Schematic example owns its product proto
and protocol generator separately from the library root.
