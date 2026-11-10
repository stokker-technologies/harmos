# Tasks

## 1. Contract

- [ ] 1.1 Settle the open points in `design.md` with the owner and record each outcome there.
- [ ] 1.2 Lock the source inventory: producers, consumers, tests, fixtures, examples and guidance.

## 2. Protocol

- [ ] 2.1 Add `crates/harmos-protocol` with the `harmos.v1` definitions, checked-in `proto/` files and a drift check.
- [ ] 2.2 Write the behaviour spec: describe, activation, invoke frames, child calls, cancellation, failures and streams.
- [ ] 2.3 Add the conformance suite with a reference host usable by non-Rust SDKs.

## 3. Host

- [ ] 3.1 Add the managed placement: start the process, assign its local endpoint, dial and supervise it.
- [ ] 3.2 Add the remote placement: connect to a URL, reconnect, and expose the start time of each activation.
- [ ] 3.3 Compute and admit the catalog digest; route child calls on the invoke call.
- [ ] 3.4 Open and close `Publish` calls per declared stream and ingest samples.

## 4. Rust SDK

- [ ] 4.1 Serve `harmos.v1` from the SDK and move sidecar serving off the framed wire.
- [ ] 4.2 Add `#[harmos::plugin]`, `harmos::PluginContext` and the target-selected `harmos::export_plugin!`.
- [ ] 4.3 Rebuild observers as SDK code over streams and share `publish_on` and its error type.
- [ ] 4.4 Migrate the examples and fixtures.

## 5. Removal

- [ ] 5.1 Delete the framed wire, handshake, credits, control pipe and binary carriers.
- [ ] 5.2 Delete publication leases, admission, cadence reports and the native observer runner.
- [ ] 5.3 Migrate the runtime sidecar behaviour tests and delete the transport tests and benches.
- [ ] 5.4 Update the product guide, README and authoring guidance.

## 6. Guests

- [ ] 6.1 Move guest command and child envelopes to the `harmos.v1` messages and delete the Postcard envelope codec.

## 7. Final Validation

- [ ] 7.1 Run targeted transport, lifecycle, conformance and guest tests on Linux and Windows.

## Workflow Follow-Up

- Dokime adopts the merged revision in its own `grpc-plugins` change.
- Delivery uses targeted validation and merges to dev without waiting for CI; breakage is fixed in follow-ups.
- SDK system tests run in Nightly only, not on pull requests.
