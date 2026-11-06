# Tasks

## 1. Prototype

- [ ] 1.1 Capture the transport baseline from the existing transport benchmarks and Dokime ingest profiling.
- [ ] 1.2 Implement a private authenticated local gRPC endpoint on Linux and Windows.
- [ ] 1.3 Move one small real plugin path to the prototype, control and data planes included.
- [ ] 1.4 Measure every gate on both platforms and record the decision in design.md.

## 2. Migration

- [ ] 2.1 Add `crates/harmos-protocol`, checked-in `proto/` files and a drift check.
- [ ] 2.2 Write the behaviour spec: handshake, versioning, lifecycle, credits, cancellation, errors and deadlines.
- [ ] 2.3 Add the conformance suite with a reference host usable by non-Rust SDKs.
- [ ] 2.4 Move the Rust SDK and runtime supervision to the protocol.
- [ ] 2.5 Delete the framed stdin/stdout wire, its handshake and related adapters.
- [ ] 2.6 Adopt the merged revision in Dokime and migrate its host adapter.

## 3. Final Validation

- [ ] 3.1 Run targeted transport, lifecycle and conformance tests and review required-term scans.

## Workflow Follow-Up

- Delivery uses targeted validation and merges to dev without waiting for CI; breakage is fixed in follow-ups.
- SDK system tests run in Nightly only, not on pull requests.
