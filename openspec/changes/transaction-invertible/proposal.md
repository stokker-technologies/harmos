# Transaction Invertible

## Why

`Invert<S>` hid the undo type in a separate `Undoable` declaration, so the trait an author implements did not say what it returns. Two small follow-ups (the plugin layout rule and the stale `authoring-streamline` change) ride along.

## What Changes

- Clean break: `harmos::state::Invert<S>` becomes `Invertible<S>: Transaction<S>` with `type Inverse: Transaction<S>` and `fn inverse(&self, before: &S) -> Self::Inverse`; the public `Undoable` trait is removed.
- `#[harmos::transaction(id = …, inverse = Other)]` emits the S-free pair declaration and binds the catalog dispatch to `Invertible<S, Inverse = Other>`; `inverse = Self` stays the common case.
- Infer irreversibility from absence of `Invertible<S>`; remove the explicit attribute and obsolete refusal fixture.
- Migrate built-ins, tests, the schematic example and the authoring skill.
- Verify the `harmos-sdk` gRPC transcode hashed-field test on dev; confirm the plugin layout rule in `harmos-architecture`.

## HF1 Follow-Ups

Merge the shared Lifecycle/Setup design, align consumer plugin structure, move
benchmark registration behind module declarations, and share retained-write
capacity waiting. Verify facade-path execution bounds and gRPC nested spans;
make the schematic story sequence backend startup through readiness. Exact-pin
published siblings and refresh dependencies/tools. Shared Hub release defects
are reported under HF1's no-sidequest limit; no push or broad validation is authorized.

## Capabilities

### New Capabilities

- `transaction-authoring`: how a transaction declares and describes its inverse.

### Modified Capabilities

None.

## Impact

`harmos-domain`, `harmos-runtime`, `harmos-macros`, the schematic example and the authoring skill. Consumers rename `Invert` to `Invertible` and move `type Inverse` into the impl. No storage or wire changes.
