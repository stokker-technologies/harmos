# Transaction Authoring

## ADDED Requirements

### Requirement: Invertible Transactions

A transaction SHALL be reversible exactly when it implements `Invertible<S>`. The catalog SHALL infer an undo barrier from absence of that implementation. An optional `inverse = Self` or `inverse = Other` declaration SHALL agree with `Invertible<S>::Inverse` and its mutual pair.

#### Scenario: Paired Inverse

- **WHEN** a transaction declares `inverse = Withdraw` and implements `Invertible<Ledger>` with `type Inverse = Withdraw`
- **THEN** undo records a `Withdraw` built against the state before the change

#### Scenario: Mismatched Body Refused

- **WHEN** a transaction declares `inverse = Withdraw` but its `Invertible` body names another `type Inverse`
- **THEN** catalog dispatch does not compile

#### Scenario: Inferred Reversibility

- **WHEN** a transaction implements `Invertible<Ledger>` without inverse metadata
- **THEN** catalog dispatch constructs its inverse

#### Scenario: Inferred Barrier

- **WHEN** a transaction does not implement `Invertible<Ledger>`
- **THEN** its committed entry blocks undo without an explicit marker
