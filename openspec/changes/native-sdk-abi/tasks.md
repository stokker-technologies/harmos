# Native SDK ABI Tasks

## Implementation

- [x] 1.1 Domain and authoring macros: one name-derived field-tag rule, shared schema JSON/Postcard codec and meaningful conversion tests.
- [x] 1.2 SDK and sidecar: encoded wire registration, lifecycle and runtime stream publication; preserve typed Rust behavior.
- [x] 1.3 FFI: opaque handles, callback ownership, Arrow transfer, panic barrier and generated C header.
- [x] 1.4 C++: header-only C++20 reflection, exceptions, CMake package with an installed-consumer proof and simulated bench example.
- [x] 1.5 Tooling: pinned CMake/nanoarrow, focused build/story tasks and inert catalog generation.
- [x] 1.6 Proof source: native host story and ABI panic containment test.
- [x] 1.7 Documentation: prototype guide, README impact and accurate limitations.

## Validation and Delivery

- [x] 2.1 Run the focused C++ system story and panic test.
- [x] 2.2 Run focused existing Rust sidecar tests and check:rust.
- [x] 2.3 Run Linux proof if the repository container is ready; otherwise record exactly what is missing.
- [x] 2.4 Scan CLEAN before each normal push; verify remote SHA; clean native Cargo outputs and retain worktree by 16:30.

## Workflow Follow-Up

- The optional repository convention scan still reports repository-wide findings; its output is retained under target/cpp/conventions.log for repair.
- Dokime's Python SDK adopting the shared core is separate follow-up work.
- Branch prototype only; no PR, merge, full simulation or full test lane.
