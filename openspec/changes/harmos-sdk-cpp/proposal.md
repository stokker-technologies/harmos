# Harmos SDK C++

## Why

Plugin authors should be able to write sidecars in C++ against the language-neutral protocol.

## What Changes

- Add `sdk/cpp` with sidecar interfaces generated from the checked-in `.proto` files.
- Provide operation registration, plugin metadata, lifecycle, observers, streams and batched data.
- Pass the conformance suite on Linux and Windows.
- CMake source distribution, built with the host's compiler. gRPC for C++ is heavy to build, so CI caches it.

## Locked Decisions

- Depends on `plugin-grpc-protocol` passing its gates and migrating.
- Sidecars only. Guests are a later decision.

## Non-Goals

Test-domain concepts, which belong to Dokime's SDK.

## Distribution

- A versioned source archive with generated protocol code on each GitHub release, with its hash.
- Projects pull it through CMake `FetchContent`, pinned by URL and hash. No binary SDK, because
  C++ binaries depend on the compiler and its settings.
- gRPC and protobuf come through a vcpkg manifest in the starter.
- mise pins CMake and Ninja; the compiler comes from the system.
- A vcpkg port or Conan package can follow once stable releases exist.
- Authors ship a `.dokime` bundle per platform, statically linked where possible.
