## 1. Description

- [x] 1.1 Add the lane list and the per-operation lane to the plugin description model and its fingerprint inputs; regenerate every binding through the generation tasks.
- [x] 1.2 Validate declarations at admission with actionable errors naming the lane and the operation.

## 2. Host

- [x] 2.1 Create a plugin's lanes scoped to its instance at admission; release them on stop and replacement.
- [x] 2.2 Admit every plugin invocation through its operation's lane with the existing lane admission; keep waiters ordered and cancellable.
- [x] 2.3 Remove the single host-chosen lane for plugin operations and its call sites.

## 3. SDKs

- [x] 3.1 Expose lane declaration and per-operation assignment in the Python plugin layer.
- [x] 3.2 Expose the same in the Rust, C# and C++ plugin layers, idiomatic to each.

## 4. Validation and Delivery

- [x] 4.1 Deterministic stories for every scenario, using barriers instead of timing.
- [ ] 4.2 Hosted validation after delivery.
- [x] 4.3 Reject invalid lane capacity overrides before mutation and freeze validated lane sets without repeated validation.
- [x] 4.4 Isolate lane conformance from unrelated plugin identity faults while preserving the malformed-identity pass count.
