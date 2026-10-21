---
name: harmos-testing
description: Author Harmos engine, runtime, carrier and packaging stories using the canonical fixture and boundary owners.
---

# Harmos Testing

Apply `stkr-testing` for layers, meaningful stories and deadlines. These
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

Each crate has at most one integration binary. Layers share the project-wide caps
in `stkr.toml` (`[conventions.testing.limits]`: unit 512, integration 256,
acceptance 128, system 64). Integration domains mirror production modules beneath
`tests/integration/main.rs`; shared controllable backends live in `tests/support/`.
Storage manager stories exercise its public engine boundary.
Their real `WorkerOwner` runs the canonical worker and joins it on close; a host
runtime is needed only when the story proves host composition. Runtime storage
stories own managed journal projection, operation dispatch and host lifecycle.
Keep source-private kernel invariants beside their production owner.

File journal fault stories use the public `FileBackend` integration boundary.
`with_journal_opener` supplies controlled real file access while the backend
retains journal parsing, rollback, exclusive leasing and close/reopen ownership.
Keep filesystem faults out of unit tests.

## Cross-Artifact Compositions

`harmos-runtime/tests/system/main.rs` composes the host baseline, schematic guest
and schematic sidecar walkthroughs. Schematic is the shipped example and the
shared system fixture, in its separate workspace. Test unary and streaming
behavior through those real artifacts; do not add a shipped operation solely to
inject a test fault.

The dedicated sandbox guest retains guest-specific policy and cleanup behavior
that schematic does not declare. Loading that built component inside Wasmtime is
a system boundary because it composes a host and a plugin artifact. Keep pure native command construction beside
the sidecar owner. The schematic sidecar system scene owns effective environment
clearing, pre-readiness process failure, cleanup, withdrawal and fresh-generation
recovery. Its host runs with a poisoned worker-count variable; a valid multithread
sidecar can recover only when the actual attachment clears that ambient value.
Do not claim this startup failure proves a crash after readiness.

## Fixture Inputs

`build` produces packageable native/Wasm outputs under
`target/build/<profile>/<target>/`; Product selectors and byte hashes live in
`product/examples/catalog.json`. `build:tests` consumes those matching Product
bytes, builds test harnesses and compile-contract inputs, exports
`tests/archive.tar.zst` and stages test-only inputs under `fixtures/`.
`tools/python/src/harmos_tools/build/fixtures.py` records the Cargo messages,
metadata and byte stamps `cargo harmos` collection requires. The Tests producer
writes `fixtures/stamp.json`: its `metadata` selects artifacts, libraries and
offline inputs, while `files` names every stamped input with its hash. Inert
macro inventory inspection uses hash-verified copies of the two Product plugin
artifacts under `fixtures/inspection/`; integration does not execute them or
download Product hosts.
Guest SDK integration inspects those inert component declarations and WIT worlds;
its sandbox inspection reuses the existing Tests-owned sandbox bytes. Stories
that instantiate either component with Wasmtime live in the guest system target.

Unit consumes the shared Rust archive. Integration consumes the archive and its
Tests-owned fixtures. Acceptance declares the Cargo subcommand and both plugin
Product files; system declares the Schematic CLI and both plugins. The native
runner verifies matching selected hashes and archive inputs. Project preflight
prepares only the selected lane's metadata and executable modes; compiler
libraries and offline inputs belong to acceptance preparation.

Use `harmos-runtime/tests/support/fixtures.rs` for artifacts, libraries and offline inputs.
Resolve package-owned executables at run time through `NEXTEST_BIN_EXE_<name>`,
falling back to the compile-time `CARGO_BIN_EXE_<name>` only under plain
`cargo test`, so stories run from the archive on another machine. Do not
scan a host's target directory or choose an arbitrary cached artifact. Preserve
collected build JSON bytes; the typed packaging API relocates paths while
retaining hashes. Preflight checks the stamp before a lane starts.

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

Acceptance preflight installs the starter toolset once before nextest starts.
Parallel starter stories disable automatic mise installs so they cannot repair
shared runtime links while another story uses them.

Python binding interoperability is Rust-driven acceptance using the tooling project's
`tools/python/.venv`. Generated Python packages remain consumer-owned; repository tooling has
no Python test suite or independently published client distribution.
