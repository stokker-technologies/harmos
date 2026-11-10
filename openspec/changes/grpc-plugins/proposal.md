# gRPC Plugins

Status: locked by the owner on 2026-11-10; implementation has not started and waits for the
owner's go.
Replaces `plugin-grpc-protocol`. The Dokime change `grpc-plugins` is the product side.

## Why

The native plugin protocol is framed Postcard over stdin and stdout, which in practice only Rust
can speak, and it only reaches a process the host starts. Plugins in other languages, and plugins
that run somewhere else, need one language-neutral contract. Harmos stays generic: it learns
nothing about tests or any product concept.

## What Changes

- One protocol package, `harmos.v1`, with one service: `Plugin { Describe, Invoke, Publish }`.
- gRPC becomes the only sidecar protocol. The framed wire, its handshake, credits, control pipe
  and adapters are deleted.
- The host always dials the plugin. A managed plugin is a process the host starts, which
  announces its loopback address; a remote plugin is a URL.
- `Describe` returns the plugin's identity, its start time and its catalog with a digest computed
  over the catalog contents.
- Plugin-to-host operation calls travel back on the `Invoke` call that started them.
- The host opens one `Publish` call per declared stream; the plugin sends one sample per message.
  An observer is a function that answers the host's ticks with one sample each.
- The host owns the only clock. Every wait an SDK offers is served by the host, so a test can
  advance time for the host and every plugin together.
- One authoring surface for sidecars and guests: derives for data and plain builder values for
  plugins, operations, streams and observers, with `serve` selecting the transport by build
  target. The plugin, observer and catalog attribute macros are removed.
- A `Stop` call lets a plugin release what it holds before the host closes.
- `sdk/<language>` holds each language's SDK over `harmos.v1`. The pilot covers Rust, Python,
  C# and C++.

## Locked Decisions

- No prototype phase and no throughput gate. The design favours simplicity; performance is tuned
  afterwards against measurements.
- A reconnect is a new activation. Harmos has no resume concept.
- Visibility has two levels: `Private` (the declaring plugin and the host) and `Public`
  (everyone connected). The `Host` level is removed.
- Protobuf is the only payload encoding, and calls between plugins are typed. There is no
  dynamic call layer.
- Each plugin connects independently: no dependency-ordered start, no reload plans. A call to an
  absent provider is refused at call time.
- Connected plugins trust each other: grants and provider authorization are removed.
- The connection is the generation: generation tokens and stale-call fencing are removed.
- Plugin endpoints are plain gRPC without encryption or credentials, so a server written by
  hand from the `.proto` files is a valid plugin.
- Shared concepts have the same name in Harmos and in a product's SDK, and the old alias
  variants of the Rust authoring macros are removed.
- Some authoring mistakes are reported when a plugin starts instead of at compile time.
- Guests stay Wasm. Their envelopes move to the `harmos.v1` messages as a second step, after the
  sidecar wire lands.
- Authoring parity between sidecar and guest is a soft target, Rust first.
- Postcard stays as the private storage encoding and never appears on the protocol.

## Impact

- Breaking for every sidecar: it must be rebuilt against the new SDK.
- Supersedes `plugin-grpc-protocol` and absorbs `harmos-sdk-layout` (both deleted with this
  change): `crates/harmos-sdk` moves to `sdk/rust`. `harmos-sdk-go` is removed; Go follows the
  pilot.
- `operation-visibility` stays its own change and ships in the same revision; this change then
  removes its `Host` level.
  `runtime-observer-cadence` is landed; its rule that an observer's interval may come from
  plugin settings carries over to SDK observers.
