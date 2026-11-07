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

System facts retain a necessary derived-revision carrier for opaque payloads; typed deactivation facts derive from their payload schema. Failure codec prefixes remain gRPC status-detail framing. The JSONL group marker is validated journal framing and remains explicit. Typed object definitions also derive through Identified; object value sequence tokens retain ordering because they identify stored updates, not data shapes. Starter templates, C++ headers and installed skills contain no remaining authored data-shape version surface. JSONL recovery now refuses every undecodable member rather than skipping records. Hosted build diagnostics exposed a missing RecoveryPosition import and an existing JournalOpener type-complexity warning; both are repaired without weakening checks.
