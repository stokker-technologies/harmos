# Design

The owner approved this additive extension on 2026-11-07. Dokime's evidence is in
`openspec/changes/dokime-python/design.md`, Authoring Integration. This worktree
is `harmos.wt/runtime-bindings`, branch `feat/runtime-defined-operations`, based
on fresh `origin/dev`; the primary checkout's unrelated lockfile is untouched.

Runtime-owned bindings retain a validated `Contract`, a decoding-memory estimator
and a native-instance preparation closure. They enter `Bindings` and reuse the
existing prepared-native dispatch, admission and settlement. The closure is a
codec/binding boundary, not an execution-policy owner. No compiled declaration
is mutated. Existing typed `Invocation` over `OperationHost` remains unchanged.

Encoded retry overrides apply to one submission, including retained callback
handles and child contexts. Existing replay-safety validation, deadline accounting
and attempt recording remain authoritative. Public observer staging retains the
admitted plugin scope and resource snapshot.

Validation is focused tests, `check:rust` and lint as explicitly requested; no
full simulation. Publication is one immediately squash-merged PR into Harmos dev.
Only Dokime's feature branch adopts the published revision. The paired worktree
is cleaned and retired after adoption, as explicitly authorized.

`Contract::from_schema` computes the owned graph's fingerprint in Harmos, avoiding
a Python identity implementation. The minimal binding supports unary canonical
encoded payloads; endpoint streams and gRPC projection are explicitly refused.
`submit_resident_in` becomes public alongside staging because an embedded owner
must activate over the same supplied resource identity. README claims already
cover embedded operation and plugin support; detailed API guidance is updated in
`docs/product-guide.md`.

Windows targeted proof passes all 50 selected operation-wire integration cases,
including four new runtime-owned binding/observer cases. `check:rust` passes its
workspace, native example, Wasm example and sandbox Clippy selections, formatting
and dependency checks. Python Ruff and type lint pass. The required-term diff scan
is empty. No full simulation ran, as instructed.
