# Observer Collection Implementation Tasks

## 1. Accepted Design

- [x] Verify observer, window, demand and failure behaviour and record accepted design.

## 2. Runtime and Wire

- [x] Extend declarations and Samples frames and regenerate Rust bindings.
- [x] Group demand, intervals, windows, scoped suppression and ingestion.
- [ ] Prove grouped ticks, silent/multiple rows, timestamps and failures.

## 3. Rust SDK

- [x] Add typed Collection output and registration through the observer owner.
- [x] Implement first/last listener hooks and cancellation cleanup.
- [ ] Prove authoring and overlapping demand deterministically.

## 4. Other SDKs

- [ ] Regenerate Python, C# and C++ bindings and assess thin authoring support.

## 5. Proof

- [ ] Format, compile touched and workspace test targets, and run focused tests.
- [ ] Run tooling tests, generation drift, repository and strict OpenSpec checks.
