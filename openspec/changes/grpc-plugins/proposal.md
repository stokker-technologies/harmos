# gRPC Plugins

Status: shape agreed with the owner on 2026-11-10; implementation waits for the owner's lock.
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
- The host always dials the plugin. A managed plugin is a process the host starts and gives a
  local endpoint; a remote plugin is a URL.
- `Describe` returns the plugin's identity, its start time and its catalog with a digest computed
  over the catalog contents.
- Plugin-to-host operation calls travel back on the `Invoke` call that started them.
- The host opens one `Publish` call per declared stream; the plugin sends one sample per message.
  An observer is SDK code that calls a function on an interval and sends the result on a stream.
- One authoring surface for sidecars and guests: `#[harmos::plugin]`, `harmos::PluginContext` and
  `harmos::export_plugin!`, selected by the build target.
- `sdk/<language>` holds each language's SDK over `harmos.v1`.

## Locked Decisions

- No prototype phase and no throughput gate. The design favours simplicity; performance is tuned
  afterwards against measurements.
- A reconnect is a new activation. Harmos has no resume concept.
- Visibility stays `Private`, `Public` and `Host`. No hidden level is added.
- Protobuf is the only payload encoding, and calls between plugins are typed. There is no
  dynamic call layer.
- Each plugin connects independently: no dependency-ordered start, no reload plans. A call to an
  absent provider is refused at call time.
- Connected plugins trust each other: grants and provider authorization are removed.
- The connection is the generation: generation tokens and stale-call fencing are removed.
- Guests stay Wasm. Their envelopes move to the `harmos.v1` messages as a second step, after the
  sidecar wire lands.
- Authoring parity between sidecar and guest is a soft target, Rust first.
- Postcard stays as the private storage encoding and never appears on the protocol.

## Impact

- Breaking for every sidecar: it must be rebuilt against the new SDK.
- Supersedes `plugin-grpc-protocol` (deleted with this change). `harmos-sdk-go` now depends on
  this change. `harmos-sdk-layout` is unchanged and still places the SDKs.
- `runtime-observer-cadence` and `operation-visibility` touch the same code and are reconciled
  before implementation starts (`design.md`, Open Points).
