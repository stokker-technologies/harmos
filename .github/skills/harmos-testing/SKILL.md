---
name: harmos-testing
description: Author Harmos engine, runtime, carrier and packaging stories using the canonical fixture and boundary owners.
---
<!-- Rendered by stkr project update from .agents/skills/harmos-testing/SKILL.md; sha256:bd789a5dc0ffdca8aa1e7ffd33e48de5fd87f05000533f80f24bd92702ff2fb1 -->

# Harmos Testing

Apply `stkr-testing` for layers, meaningful stories, deadlines and evidence. These
product boundaries determine where Harmos behavior belongs.

## Story Owners

| Owner | Behavior |
| --- | --- |
| `harmos-domain` | Portable declarations, message codecs and schema invariants. |
| `harmos-storage` | Arrow and signal kernels, persistence, paging, query transforms, retention, capacity and typed reads. |
| `harmos-runtime` | Application composition, operation/state/stream management, plugin activation and runtime storage bridges. |
| `harmos-sidecar` | Native SDK lifecycle, in-process transport and actual process attachment ownership. |
| `harmos-guest` | The guest SDK and its component contract exercised inside Wasmtime. |
| `harmos-packaging` | Artifact collection, receipt verification, bundles, bindings and generated-project authoring. |
| `harmos-macros` | Inert declaration fragments and compiler-facing authoring diagnostics. |

Each crate has one integration binary; its modules retain their domain fixtures.
Runtime domains enter through `tests/integration/main.rs`. Storage engine stories
live in `tests/engine/`, sharing the controllable backend in `tests/support/`.
Their real `WorkerOwner` runs the canonical worker and joins it on close; a host
runtime is needed only when the story proves host composition. Runtime storage
stories own managed journal projection, operation dispatch and host lifecycle.
Keep source-private kernel invariants beside their production owner.

## Cross-Artifact Compositions

`harmos-runtime/tests/system/main.rs` composes the host baseline, schematic guest
and schematic sidecar walkthroughs. Schematic is the shipped example and the
shared system fixture, in its separate workspace. Test unary and streaming
behavior through those real artifacts; do not add a shipped operation solely to
inject a test fault.

The dedicated sandbox guest retains guest-specific policy and cleanup behavior
that schematic does not declare. Loading that component inside Wasmtime is an
in-process integration boundary. Keep pure native command construction beside
the sidecar owner. The schematic sidecar system scene owns effective environment
clearing, pre-readiness process failure, cleanup, withdrawal and fresh-generation
recovery. Its host runs with a poisoned worker-count variable; a valid multithread
sidecar can recover only when the actual attachment clears that ambient value.
Do not claim this startup failure proves a crash after readiness.

## Fixture Inputs

`build` owns the root nextest archive and standalone native/Wasm compilation.
`tools/python/src/harmos_tools/build/fixtures.py` collects the original compiler
messages and receipts, stages verified outputs under `target/fixtures/`, and
uses the Hub's canonical producer stamp. Product selectors live in its
`metadata`; original producers and expected hashes retain their identity.

Use `tests/support/fixtures.rs` for artifacts, libraries and offline inputs.
Use nextest's binary environment for package-owned fixture executables. Do not
scan a host's target directory or choose an arbitrary cached artifact. Preserve
original build receipt bytes; the typed packaging API relocates paths while
retaining hashes. Preflight verifies fixtures before a lane starts.

## Authoring Acceptance

Compiler invocation is the behavior under test for generated-project check and
`test:unit`, actual development rebuild/cancellation, and public macro diagnostics.
Keep those scenarios in acceptance. Ordinary integration fixture setup consumes
built artifacts and never invokes Cargo or rustc.

Generated projects use their actual starter manifests and locks with a build-owned
Cargo vendor directory and hash-checked Python wheelhouse. The wheelhouse covers
root tooling and both starter locks. Use an isolated Cargo home and supported UV
configuration with offline mode, `no-index` and `find-links`; do not depend on a
host package cache. Rebuild stories observe product events and actual artifact
bytes, then settle their owned descendants.

Python binding interoperability is Rust-driven acceptance using the pinned root
`.venv`. Generated Python packages remain consumer-owned; repository tooling has
no Python test suite or independently published client distribution.
