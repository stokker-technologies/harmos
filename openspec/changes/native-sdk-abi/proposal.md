# Native SDK ABI

## Why

Owner decision, 2026-11-07: Rust is the nominal plugin authoring language. Other
languages translate thinly over one Rust core. Assess a real C++20 sidecar first.

## What Changes

- Add runtime-defined unary sidecar registrations and publication descriptors.
- Share schema-driven JSON/Postcard conversion in harmos-domain.
- Add unpublished harmos-ffi with a generated, checked-in C header and a C++20
  header-only SDK, CMake package and simulated bench example.
- Prove the boundary with a focused native host story and panic containment.

## Capabilities

### New Capabilities

- `native-sdk-abi`: Runtime-defined plugins through the existing framed sidecar.

## Impact

Domain schema, SDK wire table, sidecar kit, FFI, C++ SDK, focused tasks and docs.
No wire changes, gRPC work, Go or Dokime edits. Branch prototype only: commit and
push; no PR or merge. Stop by 16:30 Europe/Zurich and preserve incomplete work.
