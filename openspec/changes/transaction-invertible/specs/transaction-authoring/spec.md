# Transaction Authoring

## ADDED Requirements

### Requirement: Invertible Transactions

A transaction SHALL declare its undo answer in its attribute (`inverse = Self`, `inverse = Other` or `irreversible`), and a declared inverse SHALL be described by `Invertible<S>`, whose `type Inverse` names the declared change.

#### Scenario: Paired Inverse

- **WHEN** a transaction declares `inverse = Withdraw` and implements `Invertible<Ledger>` with `type Inverse = Withdraw`
- **THEN** undo records a `Withdraw` built against the state before the change

#### Scenario: Mismatched Body Refused

- **WHEN** a transaction declares `inverse = Withdraw` but its `Invertible` body names another `type Inverse`
- **THEN** catalog dispatch does not compile
