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

Catalog dispatch detects `Invertible<S>` against its concrete state using method resolution: a probe implements inverse construction when the trait exists and otherwise borrows a fallback returning no inverse. An implementation without `inverse = …` metadata is reversible. The optional pair declaration still checks mutuality, body equality and catalog membership. The explicit `irreversible` attribute and the unanswered-undo refusal fixture are removed.

## Alternatives

Keeping `Undoable` as a public supertrait would duplicate `Inverse` across two traits and make `Self::Inverse` ambiguous; it was rejected.
