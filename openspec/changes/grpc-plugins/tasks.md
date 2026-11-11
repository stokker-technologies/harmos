# Tasks

## 1. Contract

- [ ] 1.1 Settle the open points in `design.md` with the owner and record each outcome there.
- [x] 1.2 Lock the source inventory in design.md: producers, consumers, tests, fixtures, examples, guidance and removal paths.

## 2. Protocol

- [x] 2.1 Add `crates/harmos-protocol` exposing the `harmos.v1` definitions as `harmos_protocol::v1`, with checked-in `proto/` files and generated Rust, and a drift check that needs no installed `protoc`.
- [x] 2.2 Write the behaviour spec: describe, activation, invoke frames, child calls, cancellation, failures and streams.
- [x] 2.3 Add the conformance suite with a reference host usable by non-Rust SDKs.
- [x] 2.4 Canonicalize file options across SDK fingerprints, share Rust clock descriptors and pin both built-in clock fingerprints in conformance.

- [x] 2.5 Extend conformance and every language fixture for schema graphs, setting-only observers and canonical descriptor metadata.
- [ ] 2.6 Consolidate wire definitions in proto/generated protocol types; put only light Rust helpers in protocol and specify cross-language helper semantics in words with matching SDK implementations.

## 3. Host

- [x] 3.1 Add the managed placement: start the process, read the address it announces, dial and supervise it, and call `Stop` before closing.
- [x] 3.2 Add the remote placement: connect to a URL, reconnect, and expose the start time of each activation.
- [x] 3.3 Compute and admit the catalog digest; route child calls on the invoke call.
- [x] 3.4 Open and close `Publish` calls per declared stream, send ticks for interval streams and ingest samples.
- [x] 3.5 Lift the simulation clock into one clock per host with a monotonic and a date reading, and move every time read and wait in the runtime, storage, the client-facing gRPC layer and the built-in operations onto it.
- [x] 3.6 Serve `harmos.clock.sleep` and `harmos.clock.now`, and drive observer ticks from host timers.
- [x] 3.7 Add the virtual mode: its own timer queue, activity accounting for calls, ticks and host work, and a controller that advances while a condition is unmet, with simulation's existing tests kept passing.
- [x] 3.8 Reduce visibility to `Private` and `Public`.
- [x] 3.9 Prove Python conformance plugin admission and host-clock sleep through the real runtime under virtual time.
- [ ] 3.10 Before Dokime's Harmos-adoption wave 2, extend client-facing gRPC declarations/codecs to accept generated protobuf types and authoritative descriptors directly; preserve method paths, fingerprints, decoding-memory admission, cancellation, backpressure and all host streaming envelopes.
- [ ] 3.11 In the host wave, migrate built-in request/results to protocol messages with declaration, gRPC mapping, callee policy and handler in each operation file; register directly through narrow resources without a runtime dependency or per-service adapters.
- [ ] 3.12 Prove generated protobuf unary and streaming declarations through real gRPC routing, bounded decoding and the client authorization hook; provide the adoption example for Dokime's host-wave operations migration.

## 4. Rust SDK

- [ ] 4.1 Make `sdk/rust` the `harmos` product crate with plugin authoring and client APIs in `harmos::prelude::*`; keep SDK dependencies on protocol, move host embedding to runtime, migrate consumers and delete the old facade root and temporary SDK package names.
- [x] 4.1.1 Implement the native gRPC plugin SDK in `sdk/rust` with conformance fixtures.
- [ ] 4.2 Integrate the builder surface across host consumers and guests with target-selected `serve`; delete plugin, observer and catalog macros and their sidecar and guest variants.
- [x] 4.2.1 Add the Rust Message derive and native plugin, operation, stream and observer builder values.
- [x] 4.3 Implement native plugin SDK observers answering host ticks, route its waits through the host clock, and share publish_on and its error type.
- [ ] 4.4 Migrate the examples and fixtures.
- [ ] 4.5 Move surviving native invocation, resource and serving engine code from `crates/harmos-sdk` to host owners with narrow resources for operations; retain guest transport in the SDK and guest loading in runtime.
- [ ] 4.6 Verify plugin and client SDK dependency closures contain no host crates; migrate example/build-script imports to protocol or runtime as appropriate, and update README/product guidance for the public entry point.

## 5. Pilot SDKs

- [x] 5.1 Python: `sdk/python` over `harmos.v1`, passing the conformance suite.
- [x] 5.2 C#: `sdk/csharp` over `harmos.v1`, passing the conformance suite.
- [x] 5.3 C++: `sdk/cpp` over `harmos.v1`, passing the conformance suite.

## 6. Removal

- [ ] 6.1 Delete the framed wire, handshake, credits, control pipe and binary carriers.
- [ ] 6.2 Delete publication leases, admission, cadence reports and the native observer runner.
- [ ] 6.3 Delete the plugin graph, start ordering and reload plans; grants and provider selection; generation tokens and stale-call fencing.
- [ ] 6.4 Delete `crates/harmos-ffi` and its remaining manifest, task and consumer references.
- [x] 6.4.1 Replace the C ABI bridge under `sdk/cpp` with the gRPC SDK.
- [ ] 6.5 Migrate remaining host/guest operation payloads and catalogs to protobuf-derived definitions and remove Postcard message encoding from the wire; the native plugin derive is covered by 4.2.1.
- [ ] 6.6 Migrate the runtime sidecar behaviour tests and delete the transport tests and benches.
- [ ] 6.7 Update the product guide, README and authoring guidance.
- [ ] 6.8 Replace domain wire mirrors with protocol types and delete dead conversions; retain only native state/recovery, schema/encoding and storage contracts listed in Crate Structure, moving dispatch/resource code to its engine owner.
- [ ] 6.9 Redirect contract-only operations consumers to protocol/SDK, then remove the operations `runtime` feature without moving handlers out of their operation files; remove the superseded host SDK root after its consumers migrate.

## 7. Guests

- [ ] 7.1 Move guest command and child envelopes to the `harmos.v1` messages and delete the Postcard envelope codec.

## 8. Final Validation

- [ ] 8.1 Run targeted transport, lifecycle, conformance and guest tests on Linux and Windows.

- [ ] 8.2 Run each pilot SDK against the extended conformance suite on native Linux and Windows; retain failures and repair affected consumers.

## Workflow Follow-Up

- Dokime adopts the merged revision in its own `grpc-plugins` change.
- Dokime wave 2 adopts 3.10; host wave 3 moves server behaviour to operations with direct registration and deletes each area's `hosted!`/`Host`/`Serve`; waves 4–5 redirect CLI/desktop hosting to runtime and delete server/domain crates and dead conversions as specified in Crate Structure.
- After the last gRPC wave, Harmos `storage-interface` implements and benchmarks the generic extensions; Dokime's change of the same name consumes them, extracts application persistence and supplies before/after multi-day benchmarks. No storage extension or extraction runs in a gRPC wave.
- Delivery uses targeted validation and merges to dev without waiting for CI; breakage is fixed in follow-ups.
- SDK system tests run in Nightly only, not on pull requests.
