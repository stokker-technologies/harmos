# Harmos SDK Go

## Why

Plugin authors should be able to write sidecars in Go against the language-neutral protocol.

## What Changes

- Add `sdk/go` with sidecar interfaces generated from the checked-in `.proto` files.
- Provide operation registration, plugin metadata, lifecycle, observers and streams.
- Pass the conformance suite on Linux and Windows.
- A Go module under `sdk/go`, released with subfolder tags.

## Locked Decisions

- Depends on `grpc-plugins` landing the `harmos.v1` protocol.
- Sidecars only. Guests are a later decision.

## Non-Goals

Test-domain concepts, which belong to Dokime's SDK.

## Distribution

A Go module under `sdk/go`, installed with `go get` and released with subfolder tags. mise pins Go.
