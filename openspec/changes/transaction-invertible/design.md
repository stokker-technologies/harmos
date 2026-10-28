# Transaction Invertible Design

## Trait Shape

```rust
pub trait Invertible<S>: Transaction<S> {
    type Inverse: Transaction<S>;

    fn inverse(&self, before: &S) -> Self::Inverse;
}
```

An optional `inverse = …` attribute emits the hidden S-free `Paired { type Inverse }` declaration, checking mutuality where the transaction is declared. The generated `__harmos_check_inverse` is bounded on `Self: Invertible<S, Inverse = Declared>` and catalog membership, so a mismatched body does not compile at the catalog. Catalog dispatch infers reversibility from the implementation itself.

## Irreversible Changes

Catalog dispatch detects `Invertible<S>` against its concrete state using method resolution: a probe implements inverse construction when the trait exists and otherwise borrows a fallback returning no inverse. An implementation without `inverse = …` metadata is reversible. The optional pair declaration still checks mutuality, body equality and catalog membership. The explicit `irreversible` attribute and the unanswered-undo refusal fixture are removed.

## Alternatives

Keeping `Undoable` as a public supertrait would duplicate `Inverse` across two traits and make `Self::Inverse` ambiguous; it was rejected.
