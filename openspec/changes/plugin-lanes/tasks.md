## 1. Description

- [ ] 1.1 Add the lane list and the per-operation lane to the plugin description model and its fingerprint inputs; regenerate every binding through the generation tasks.
- [ ] 1.2 Validate declarations at admission with actionable errors naming the lane and the operation.

## 2. Host

- [ ] 2.1 Create a plugin's lanes scoped to its instance at admission; release them on stop and replacement.
- [ ] 2.2 Admit every plugin invocation through its operation's lane with the existing lane admission; keep waiters ordered and cancellable.
- [ ] 2.3 Remove the single host-chosen lane for plugin operations and its call sites.

## 3. SDKs

- [ ] 3.1 Expose lane declaration and per-operation assignment in the Python plugin layer.
- [ ] 3.2 Expose the same in the Rust, C# and C++ plugin layers, idiomatic to each.

## 4. Validation and Delivery

- [ ] 4.1 Deterministic stories for every scenario, using barriers instead of timing.
- [ ] 4.2 Hosted validation after delivery.
