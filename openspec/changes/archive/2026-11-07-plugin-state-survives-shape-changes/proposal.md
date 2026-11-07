## Why

Plugin changes and removal must never invalidate a storage root. Historical plugin data must retain its written definition without migrations or resets.

## What Changes

- Retain schema graphs with plugin system facts (Guest and Sidecar), validate their stored revision, and expose the historical schema independently of installed plugins.
- Preserve generic stored records, objects and streams through their existing stored schema references. Preserve object revisions whose exact definition is absent from the current catalog during pruning.
- Keep host transaction and record catalog refusal, including snapshot inventories, unchanged.
- Document that plugin scopes register streams and listeners, not typed state records, transactions or object catalogs. Plugins write generic storage through host operations; listeners have no persisted runtime state.

## Capabilities

### New Capabilities
- `plugin-state-history`: Definition-bound plugin history independent of installed plugins.

### Modified Capabilities
- `data-shape-identity`: Scope incompatible-definition root refusal to host built-in state types.

## Impact

Domain system fact codec, storage object retention, runtime recovery tests and product documentation. Existing plugin fact roots without definitions may require a fresh root once. Schema and storage encodings remain v1. No coupled repository changes.
