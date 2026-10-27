## ADDED Requirements

### Requirement: Struct As Request

An operation declaration SHALL be its own request message. `response` SHALL default to `()`, `visibility` to `Private` with the
short forms `Shared` and `Host`, and `policy` SHALL be required. `request =`, `infer_request`, `RequestFor` and `operation_for`
SHALL NOT exist.

#### Scenario: Unit Request

- **WHEN** a unit struct declares an operation without `response`
- **THEN** it is invoked as `runtime.operation(Op).run()` and answers `()`.

### Requirement: Dotted Operation Identifiers

Local operation IDs SHALL match `segment(.segment)*` with each segment `[a-z][a-z0-9_]*`; the host SHALL prepend the owner.

#### Scenario: Rejected Segment

- **WHEN** an operation declares `id = "host..cancel"` or `id = "Host.cancel"`
- **THEN** the macro refuses the declaration.

### Requirement: Declared Resources And Operations

`resources = [..]` and `operations = [..]` SHALL be the only resources and operations an operation reaches through its context.
Undeclared use SHALL fail to compile. Registration and plugin activation SHALL refuse missing resources, missing or
incompatible operations and cross-owner operations that are not `Shared` (`Host` for a host caller). Run time SHALL only report
the operation error, deadlines and carrier faults.

#### Scenario: Undeclared Resource

- **WHEN** `run` calls `ctx.resource::<T>()` without declaring `T`
- **THEN** compilation fails.

#### Scenario: Missing Resource

- **WHEN** a batch declaring resource `T` is registered without `T`
- **THEN** registration refuses the batch and no operation is published.

### Requirement: Conditional Catalog Variants

`#[harmos::catalog]` SHALL apply each variant's attributes to the variant and to every generated projection and registration.

#### Scenario: Feature-Gated Variant

- **WHEN** a variant is gated by `#[cfg(feature = "x")]`
- **THEN** the catalog compiles with and without the feature and lists the member only when enabled.
