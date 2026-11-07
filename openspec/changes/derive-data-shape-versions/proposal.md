# Derive Data Shape Versions

## Why

Data shapes have one canonical structural identity. Authors must not maintain ordinal versions.

## What Changes

- Native streams use the existing schema/window revision rule.
- Records and transactions derive opaque schema-only revisions from the same canonical encoding.
- Listeners bind record identity and validate its row schema without authored versions.
- Remove unused group/snapshot framing ordinals and obsolete generated identity constants.
- Recovery and snapshots refuse changed stored definitions with identity and fresh-root advice.
- Preserve failure details, ABI, transport, journal, manifest, codec and package version contracts.

## Impact

Runtime, domain, macros, generated inventories, consumers, examples, C++ SDK, templates and maintained guidance are reviewed together. Development storage may break cleanly; no migration or compatibility aliases are added. The owner requires targeted tests, C++ proof, a final clean focused build, project-check finding parity and CLEAN scan before squash delivery. No full lanes or CI simulation.
