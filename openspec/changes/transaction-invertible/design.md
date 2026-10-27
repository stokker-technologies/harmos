# Transaction Invertible Design

## Trait Shape

```rust
pub trait Invertible<S>: Transaction<S> {
    type Inverse: Transaction<S>;

    fn inverse(&self, before: &S) -> Self::Inverse;
}
```

The attribute still owns the undo answer. It emits a hidden S-free `Paired { type Inverse }` declaration so the mutual pair check (`inverse = X` names this change back) runs where the change is declared. The generated `__harmos_inverse` is bounded on `Self: Invertible<S, Inverse = Declared>`, so a body returning another change does not compile at the catalog.

## Irreversible Changes

`Invertible<S>` no longer requires the attribute's declaration, so an `irreversible` change that also implements it compiles; the body is dead code because dispatch never asks an irreversible change for an inverse. A coherence guard (`impl<S> Guard<S> for X` beside a blanket over `Invertible<S>`) was tried and rejected: the generic state parameter lets downstream crates implement `Invertible<_>`, so every irreversible change conflicted. The `irreversible_body` compile fixture is removed.

## Alternatives

Keeping `Undoable` as a public supertrait would duplicate `Inverse` across two traits and make `Self::Inverse` ambiguous; it was rejected.
