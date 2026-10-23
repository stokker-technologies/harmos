---
name: harmos-testing
description: Author Harmos engine, runtime, carrier and packaging stories using the canonical fixture and boundary owners.
---

# Harmos Testing

Apply `stkr-test` for layers and lanes. These are Harmos-specific exceptions.
Run the full lanes locally on the combined tree before opening or updating a
review; CI confirms.

## Story Owners

Storage engine stories exercise `harmos-storage` without a host runtime.
Its real `WorkerOwner` owns the canonical worker and settlement.
Runtime storage stories cover host composition, managed journal projection and
operation dispatch.

File journal faults belong at the public `FileBackend` integration boundary;
controlled file access retains the backend's parsing, rollback and leasing.

Two integration targets use specialized paths: `harmos-domain` uses
`tests/message/main.rs`; `harmos-guest` uses `tests/kit/main.rs`.

## Cross-Artifact Compositions

The runtime system target composes the host baseline and the Schematic guest
and sidecar artifacts. Schematic is the shipped example and shared system
fixture in its own workspace.

The sandbox guest covers guest policy and cleanup absent from Schematic.
Instantiating a component in Wasmtime belongs to system tests; inert component
metadata and WIT inspection belong to integration.

The Schematic sidecar scene proves environment clearing, pre-readiness failure,
cleanup, withdrawal and fresh-generation recovery. Pre-readiness failure is not
evidence of a crash after readiness.

## Fixture Inputs

Tests-owned fixtures contain hash-verified inert copies of the Product plugin
artifacts for integration inspection. Sandbox inspection reuses Tests-owned
sandbox bytes. Integration does not execute those components.

The Tests stamp selects artifacts, libraries and offline inputs and records
their hashes. Runtime fixture support owns their resolution. Collected Cargo
metadata retains producer identity and byte hashes.

Acceptance selects the Cargo subcommand and both plugin Product artifacts;
system selects the Schematic CLI and both plugins.

## Authoring Acceptance

Compiler execution is the behavior under test for generated projects, public
macro diagnostics and development rebuild/cancellation, so these stories belong
to acceptance. Integration fixture setup consumes already-built artifacts.

Generated-project stories use starter manifests and locks, a build-owned Cargo
vendor directory and a hash-checked Python wheelhouse. Their isolated offline
inputs cover root tooling and both starter locks.

Python binding interoperability is Rust-driven acceptance using the root
`.venv`; generated Python packages remain consumer-owned.
