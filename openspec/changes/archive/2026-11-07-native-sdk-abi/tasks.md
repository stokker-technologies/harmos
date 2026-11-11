# Native SDK ABI Tasks

## Implementation

- [x] 1.1 Domain and authoring macros: one name-derived field-tag rule, shared schema JSON/Postcard codec and meaningful conversion tests.
- [x] 1.2 SDK and sidecar: encoded wire registration, lifecycle and runtime stream publication; preserve typed Rust behavior.
- [x] 1.3 FFI: opaque handles, callback ownership, Arrow transfer, panic barrier and generated C header.
- [x] 1.4 C++: header-only C++20 reflection, exceptions, CMake package with an installed-consumer proof and simulated bench example.
- [x] 1.5 Tooling: pinned CMake/nanoarrow, focused build/story tasks and inert catalog generation.
- [x] 1.6 Proof source: native host story and ABI panic containment test.
- [x] 1.7 Documentation: prototype guide, README impact and accurate limitations.

## Integration into Dev

- [x] 1.8 Sidecar host, dynamic catalog and host story: remove authored stream revisions and resolve against dev's derived revisions.
- [x] 1.9 ABI and C++ example: expose the core cancellation wait and replace the acquisition busy-wait; preserve ownership and panic barriers.
- [x] 1.10 Build tooling and documentation: honor eight Cargo jobs, keep C++ proof opt-in, document SDK commands and publishing targets, and remove obsolete assessment instructions.
- [x] 1.11 Domain listener validation: repair the single-pattern match warning that blocks the required build's compiler preflight without changing behavior.

## Validation and Delivery

- [x] 2.1 Pass mise run build:cpp with CARGO_BUILD_JOBS=8, lowering only on allocation failure.
- [x] 2.2 Pass mise run test:rust:cpp, including the installed consumer, real C++ demo and ABI panic test.
- [x] 2.3 Review foreign ownership, ordinary Rust CI independence and the C++-only scope; report Linux and throughput as unproven.
- [x] 2.4 Retire the superseded gRPC C++ proposal. Finish this change and perform delivery scan, PR and merge after focused proof.

## Workflow Follow-Up

- Dokime's Python SDK adopting the shared core is separate follow-up work.
- The owner authorizes immediate squash delivery to dev after a CLEAN scan. No full lanes, lint tasks or simulations; hosted CI owns broader proof.
- Future API review: nanoarrow construction verbosity and ctx.operation reaching only this plugin's registered operations. No API redesign in this delivery.
