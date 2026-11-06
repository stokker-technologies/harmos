# Implementation Tasks

## Source Inventory and Order

- [x] 1.1 Lock owner decisions and coupled ownership in proposal, design and requirements; inspect current retry safety, attempt evidence and Invocation id callers.
- [x] 1.2 Harmos SDK: implement Invocation and OperationHost options, handler adapter and associated instance; preserve Invokes authorization and metadata accessor in operations/{surface,operation,calls,instance,mod}.rs and exports.
- [x] 1.3 Harmos runtime: replace Invocation routing builder with concrete hosts and lane extension; preserve scoped admission, cancellation, metadata and defaults in operations/{surface,handler,invocation,manager,wire,children}.rs.
- [x] 1.11 Add per-attempt middleware evidence callbacks in operations/{middleware,execution}.rs and prove failed/successful attempts individually; consumer persistence is owned by the paired Dokime change.
- [x] 1.4 Harmos carriers and wire: transport retry options through sidecar/guest dispatch and WireChildCall codecs; refuse unsafe overrides at admission and preserve deadlines/evidence.
- [x] 1.5 Prove one handler and one Dokime template using the shared builder before mass migration; use an integration-only local dependency override if needed.
- [x] 1.6 Migrate all Harmos consumers, tests, examples, public exports, docs and guest template README; remove superseded APIs and migrate the sole dynamic-registration id caller to submit_at.
- [x] 1.7 Focused Harmos proof: admission, cancellation, retry/defaults/safety, child metadata, undeclared calls and sidecar/guest calls.

## Final Validation

- [x] 2.1 Close Harmos source inventory and run check:rust (including lint); scan the diff for prohibited names.

## Workflow Follow-Up

The paired Dokime change owns consumer migration, durable execution-log attempt evidence, built-in authorization, docs/reference hashes, bindings, final rebase and pin. Delivery remains authorized: push Harmos, verify its remote SHA and immediately squash its ready PR into dev; then deliver Dokime against that merged revision. Finish this change through the specification workflow after source and validation are complete. Clean compiler outputs and retire both owned worktrees and branches only after delivery evidence is preserved.
