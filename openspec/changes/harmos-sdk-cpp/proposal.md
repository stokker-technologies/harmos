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
