# gRPC Plugins

## Why

Plugins need a language-neutral carrier for managed processes, remote endpoints and
supported Wasm guests. Harmos owns transport, routing and settlement while a product
owns its plugin contract and author-facing policy.

## What Is Landed

- gRPC replaces the framed stdin/stdout sidecar protocol and its handshake/credits.
- The host dials managed loopback or remote endpoints. Get admits declarations;
  Invoke carries unary attempts, child calls and cancellation; Samples carries
  host-granted windows and columnar parts; Stop settles the carrier.
- Host-owned clocks, independent scoped bindings, bounded capture, reconnect,
  managed-process settlement and Rust/Python/C#/C++ carrier implementations exist.
- Field-reading derives remain; declaration values replace attribute macros.
  Domain, operations, runtime and protocol have separate responsibilities.

## Current Ownership

`operation-boundary` owns in-process-only host operations, the typed native facade,
builder-owned managers and removal of host-client roots. A product exposes its own
remote wrappers; the composition file and narrowing mechanism are deleted.
`plugin-boundary` owns the minimal carrier, shared helpers/codegen/conformance and
product ownership of plugin contracts, SDKs, registry, bundles and settings policy.
`plugin-lifecycle` owns register/initialize/finalize, host start/stop/run and catalog
naming. These changes are still implementing; this change does not claim their work.
Plugin analyses are removed, with no replacement plugin analysis subsystem.
Generated protocol output stays tracked and is regenerated through project tasks.

## Retained Decisions

- Every reconnect creates a new activation; stale generations cannot resume calls.
- Private plugin operations run only inside their declaring process. Protected
  operations admit the host/declaring owner; Public calls remain host-mediated.
- Plugins connect independently; missing providers refuse at call time.
- Protobuf carries plugin payloads; Postcard remains private storage encoding.
- Managed-process shutdown retains ownership through bounded settlement; remote
  detach preserves cleanup failures without terminating an unowned process.
- Sidecar and supported guest carriers share the product contract; no deprecated
  framed path, declaration aliases or compatibility adapters remain.

## Release Readiness

Source is landed for this change's retained scope. Tasks 8.1–8.2 retain the combined
native Linux/Windows transport, guest, SDK, drift and packaged-conformance proof.
Release rehearsal/publication remains in `release-assets-layout`; product adoption
is proved by the product. Source inspection does not close any of that final proof.
