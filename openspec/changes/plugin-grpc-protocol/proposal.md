# Plugin gRPC Protocol

## Why

The native plugin protocol is framed Postcard over stdin and stdout, which in practice
only Rust can speak. C++ and Go SDKs need a language-neutral contract.

## What Changes

- Prototype a gRPC sidecar transport on one small real plugin and measure it against today's.
- If the gates pass, add `crates/harmos-protocol` with the definitions, a written behaviour
  spec and a conformance suite, check in the `.proto` files under `proto/`, move the Rust SDK
  over, and delete the old framed wire code.

## Locked Decisions

- gRPC is the only sidecar protocol after migration. Keeping both permanently is excluded.
- Control uses ordinary gRPC calls. Data uses one streaming RPC carrying Arrow batches as bytes.
- Guests stay Wasm and share message definitions only. gRPC does not apply to them.
- Postcard stays as the private storage encoding and never appears on the protocol.

## Gates

Migration proceeds only if the prototype shows: the same typed requests, signals and error
context with version admission; responsive cancellation under backpressure; preserved startup
failure, reload, stale-generation and shutdown behaviour; bounded memory; sustained Arrow
throughput, small-call latency, startup and cancellation latency at least adequate for Fast
Stream Support against the measured baseline; and net code removal.

## Non-Goals

Remote plugin placement, and guest changes.
