# Plugin Boundary

## Why

Several products will embed Harmos. Its reusable plugin runtime currently also owns
installation, registry persistence, bundles and broad author SDKs, while a product
duplicates inspection and generation. The owner locked runtime ownership in Harmos
and policy plus all author-facing surfaces in products on 2026-11-16.

## What Changes

- **BREAKING.** Harmos keeps its internal plugin manager, register → admission →
  initialize → finalize machinery, process and Wasm hosting, routing, transport,
  reconnect/fencing, sampling/windows, deadlines and cancellation.
- Keep a minimal carrier protocol and thin Rust, Python, C# and C++ carrier helpers.
  Products own their plugin contracts and the only author-facing SDKs and starters;
  Harmos types do not appear in the author API. No import aliases.
- Consolidate descriptor/codegen tooling in Harmos, preserving the consuming
  product's richer renderers and fixes; add bounded metadata inspection.
- **BREAKING.** Products own installation, registry, bundles and artifact digests,
  distribution, persisted settings, admission policy and management operations.
  A registered plugin kind supplies admission and settings resolution callbacks;
  the product decides what and when to load. No `PluginBackend` trait.
- Keep declaration digests and admission mechanics generic. Split generic carrier
  conformance from product conformance and regenerate once with `operation-boundary`
  and `plugin-lifecycle`.

This amends those two changes and architecture invariants 8, 9 and 17. Their earlier
library-owned author SDK and plugin management assignments are superseded. This
documentation delivery locks direction; it does not claim implementation.

This also supersedes `operation-grammar`'s library-owned author SDK and installed
plugin management assignments; its visibility, caller and declaration integrity
contracts remain. The earlier research recommendation to keep full Harmos author
SDKs is not the locked decision; only reusable carrier helpers stay.

## Capabilities

### New Capabilities

- `plugin-runtime-boundary`: product-driven activation, kind callbacks, opaque
  handles and the shared carrier/runtime responsibilities.
- `plugin-inspection`: bounded metadata inspection using the runtime's process
  and transport ownership without installation or plugin initialization.
- `plugin-carrier-tooling`: four-language carrier helpers, shared generation and
  generic conformance without exposing Harmos as an author API.

### Modified Capabilities

None. Existing plugin history and runtime contribution guarantees remain; the
in-flight changes carry the SDK and facade amendments.

## Impact

`crates/harmos-runtime/src/plugin/`, `src/bundle/` in that crate,
`proto/harmos/{model,plugin,host}/v1`, `tools/packaging/src/bundle/`,
`tools/python/src/harmos_tools/build/`, all four `sdk/` trees,
`tools/conformance`, Schematic, fixtures, project tasks and packaging, README and
the plugin/SDK guides. Bundle and persistent management implementations move to
a product; they are removed here, not copied into a second owner.
