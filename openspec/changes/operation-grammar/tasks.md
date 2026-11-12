# Operation Grammar Tasks

## 1. Owner Lock and Source Inventory

- [ ] 1.1 Obtain the owner's lock on proposal.md's proposed choices and design.md's contract questions; verify an explicit decision record distinguishes approval from this documentation-only authoring.
- [ ] 1.2 Decide library SDK retention versus direct protocol implementations, plugin parity, generic client scope and visibility policy; verify each language's owned source, consumers and replacement conformance peers are inventoried without assuming four-language client parity.
- [ ] 1.3 Measure latest-durable visibility lag and synchronous commit contribution under the agreed workload, including backlog; verify measurements inform the lock without adding wall-clock pass/fail assertions or changing consumer budgets.
- [ ] 1.4 Lock issued-cut provenance, retention/restart, composite selection, file deletion/retained readers, watch semantics and explicit managed methods; verify every open contract affecting implementation has an owner decision before coding.
- [ ] 1.5 Lock the 45-operation and 66-provider-hook mapping, namespaces, wrappers, upload bindings, reachable message inventory and coupled consumer migration; verify counts against descriptors and record exact owned paths in this checklist before editing implementation.

## 2. Authoritative Protocol and Generation

- [ ] 2.1 Migrate `proto/harmos/v1/`, `crates/harmos-protocol/`, operation declarations and their generated bindings as one compiling slice, including response wrappers, positions/cuts, paging, timestamps/durations, typed discovery, Failure and one retry policy; verify focused schema/fingerprint/generation checks and update `proto/harmos/v1/behaviour.md` with that slice.
- [ ] 2.2 Replace unconditional twins in runtime gRPC projection and packaging/client generators with explicitly declared canonical routes; migrate affected internal callers and descriptor fixtures in the same slice, verify focused getter-route and cooperative-final-result tests, and update affected generated-client documentation.
- [ ] 2.3 Generate the static storage service and retained SDK storage bindings from authoritative exposed declarations, preserving admission and native-only boundaries; verify descriptor coverage equals the locked exposure inventory and update `docs/storage.md` with the same change.

## 3. Native Storage, Reads and Mutations

- [ ] 3.1 Migrate `crates/harmos-storage/src/backend.rs`, state recovery hooks and every provider/manager consumer to the locked mapping while preserving storage encodings, atomic publication and recovery; verify focused backend/restart tests and update affected backend-contract guidance.
- [ ] 3.2 Implement durable latest resolution and issued-cut continuation in snapshots/query and storage handlers; verify controlled-backend tests prove no read-triggered flush, exact repeatability, foreign/expired-cut refusal and bounded paging under mutation, and document durability lag in `docs/storage.md`.
- [ ] 3.3 Replace the three row-follow wire paths with one WatchRows selector and durable boundary/change/progress union; verify focused records/samples/object-revision opening-race, filter-progress and gap tests, preserving distinct file/directory watches and their documentation.
- [ ] 3.4 Implement field-mask Set and expected-revision validation in the owning mutation/backend paths, preserving atomic SetObject and immutable schema revision rules; verify selected/full replacement, conflicts and no partial mutation with focused tests and update mutation examples.
- [ ] 3.5 Replace immutable batch exchange entry points with bounded Append operations and accepted/durable receipts, preserving replay windows and admitted-completion ownership; verify lost replies, changed-payload replay, expired keys and immutable prefixes with focused tests and update SDK/storage examples.

## 4. Shared Unary Upload Sessions

- [ ] 4.1 Bind the existing production owner as the common client/plugin unary session mechanism, adding GetFileProduction status and the six locked routes; verify focused client and plugin session/chunk tests exercise one implementation and update transfer examples.
- [ ] 4.2 Migrate writer resume/fencing, ordered chunk replay, integrity checks, finish, abandon and grant enforcement; verify deterministic lost-reply, stale-writer, conflicting-chunk, reconnect and finish/abandon tests and document retained partial-data behavior.
- [ ] 4.3 Remove WriteFile's streaming upload route and every superseded caller/generator/fixture after session migration; verify no old streaming upload path remains, unsupported providers refuse explicitly, and `docs/storage.md` documents the capability boundary.

## 5. Plugin Hosts, SDKs and Independent Defects

- [ ] 5.1 Migrate retained library SDKs or their owner-approved replacements and host plugin carriers to the new names, wrappers, fingerprints and policies in one compiling slice; verify focused cross-language catalog/child-call/failure/scope/clock tests and update language guides.
- [ ] 5.2 Correct Python's Public default according to the locked visibility policy, provide C# invocation-local resources and cancellation reasons, and remove the stale draft proto header; verify focused behavioral SDK tests and contract review, with matching guide changes.
- [ ] 5.3 Preserve interval and device publication, activation fencing, connection-loss isolation, cooperative final settlement and product refusal propagation; verify focused conformance/runtime tests, including queued Cancel refusal and single-resource rename at the product-owned boundary, and update the behavior contract.
- [ ] 5.4 Migrate `tools/conformance/`, SDK fixtures, native/guest examples and affected tests from retired operation names and envelopes; verify each changed example and fixture with focused project tasks before proceeding.

## 6. Consumer Handoff and Source Closure

- [ ] 6.1 Review README, `docs/product-guide.md`, `docs/storage.md`, language tooling guides, generator examples and existing capability contracts against the implemented surface; verify affected claims/links are updated and unrelated authoring migrations remain with their owners.
- [ ] 6.2 Prepare the exact library candidate for later authorized publication/pinning and the product's host/SDK migration; verify the handoff identifies its unserved plugin-host operations and schedules desktop host/client regeneration together, without adding product resources to the library.
- [ ] 6.3 Remove superseded routes, policies, adapters, generated envelopes and documentation after all library consumers migrate; verify the locked inventory is closed in one combined candidate with focused tests and owned format/language checks complete before declaring source closure.

## 7. Final Validation

- [ ] 7.1 After source closure, run strict OpenSpec validation and the project's repository/language checks for the combined candidate; verify actual command results and keep any failures as repair obligations.
- [ ] 7.2 Under later delivery authorization, validate the combined library and pinned product candidate through configured hosted CI and Nightly on native Windows and Linux, including SDK/conformance, restart, cancellation, paging and uploads; verify exact revisions and actual suite results without treating one platform as proof of the other.
- [ ] 7.3 Review final operation/route/message counts, retained capability coverage and consumer documentation against the locked contract; verify owner acceptance and leave any unperformed proof explicitly pending.

## Workflow Follow-Up

- This authoring task commits only this proposed change locally; implementation, push, review creation, publication and specification activation or retirement are not authorized by it.
- After a later owner lock and implementation authorization, preserve this checklist as the source inventory; refine bounded tasks before editing rather than silently narrowing scope.
