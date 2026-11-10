# Tasks

## 1. Contract

- [ ] 1.1 Settle the open points in `design.md` with the owner and record each outcome there.
- [ ] 1.2 Lock the source inventory: producers, consumers, tests, fixtures, examples and guidance.

## 2. Protocol

- [ ] 2.1 Add `crates/harmos-protocol` exposing the `harmos.v1` definitions as `harmos_protocol::v1`, with checked-in `proto/` files and generated Rust, and a drift check that needs no installed `protoc`.
- [ ] 2.2 Write the behaviour spec: describe, activation, invoke frames, child calls, cancellation, failures and streams.
- [ ] 2.3 Add the conformance suite with a reference host usable by non-Rust SDKs.

## 3. Host

- [ ] 3.1 Add the managed placement: start the process, read the address it announces, dial and supervise it, and call `Stop` before closing.
- [ ] 3.2 Add the remote placement: connect to a URL, reconnect, and expose the start time of each activation.
- [ ] 3.3 Compute and admit the catalog digest; route child calls on the invoke call.
- [ ] 3.4 Open and close `Publish` calls per declared stream, send ticks for interval streams and ingest samples.
- [ ] 3.5 Lift the simulation clock into one clock per host with a monotonic and a date reading, and move every time read and wait in the runtime, storage, the client-facing gRPC layer and the built-in operations onto it.
- [ ] 3.6 Serve `harmos.clock.sleep` and `harmos.clock.now`, and drive observer ticks from host timers.
- [ ] 3.7 Add the virtual mode: its own timer queue, activity accounting for calls, ticks and host work, and a controller that advances while a condition is unmet, with simulation's existing tests kept passing.
- [ ] 3.8 Reduce visibility to `Private` and `Public`.

## 4. Rust SDK

- [ ] 4.1 Move `crates/harmos-sdk` to `sdk/rust`, keeping its crate name, and serve `harmos.v1` from it in place of the framed wire.
- [ ] 4.2 Replace attribute-macro authoring with derives for data and builder values for plugins, operations, streams and observers, with a target-selected `serve`; delete the plugin, observer and catalog macros and their sidecar and guest variants.
- [ ] 4.3 Rebuild observers as functions that answer host ticks, route every SDK wait through the host clock, and share `publish_on` and its error type.
- [ ] 4.4 Migrate the examples and fixtures.

## 5. Pilot SDKs

- [ ] 5.1 Python: `sdk/python` over `harmos.v1`, passing the conformance suite.
- [ ] 5.2 C#: `sdk/csharp` over `harmos.v1`, passing the conformance suite.
- [ ] 5.3 C++: `sdk/cpp` over `harmos.v1`, passing the conformance suite.

## 6. Removal

- [ ] 6.1 Delete the framed wire, handshake, credits, control pipe and binary carriers.
- [ ] 6.2 Delete publication leases, admission, cadence reports and the native observer runner.
- [ ] 6.3 Delete the plugin graph, start ordering and reload plans; grants and provider selection; generation tokens and stale-call fencing.
- [ ] 6.4 Delete `crates/harmos-ffi` and the C ABI bridge under `sdk/cpp`.
- [ ] 6.5 Derive protobuf definitions from operation types into the catalog and delete the Postcard message encoding on the wire.
- [ ] 6.6 Migrate the runtime sidecar behaviour tests and delete the transport tests and benches.
- [ ] 6.7 Update the product guide, README and authoring guidance.

## 7. Guests

- [ ] 7.1 Move guest command and child envelopes to the `harmos.v1` messages and delete the Postcard envelope codec.

## 8. Final Validation

- [ ] 8.1 Run targeted transport, lifecycle, conformance and guest tests on Linux and Windows.

## Workflow Follow-Up

- Dokime adopts the merged revision in its own `grpc-plugins` change.
- Delivery uses targeted validation and merges to dev without waiting for CI; breakage is fixed in follow-ups.
- SDK system tests run in Nightly only, not on pull requests.
