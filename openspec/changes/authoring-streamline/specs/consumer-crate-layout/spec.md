## ADDED Requirements

### Requirement: Concern-Based Operations Crate

Products using the agreed vertical authoring model SHALL place operations in <product>-operations, organized directly by concern. lib.rs SHALL export concern catalogs. Each operation file SHALL colocate its messages, portable definition and runtime-gated resource fields and Operation impl. The crate SHALL NOT add a redundant operations/ directory or place execution code in <product>-domain.

#### Scenario: Operation Organization

- **WHEN** a product contains temperature and motion operations
- **THEN** the operations crate uses the following concern-based layout and exports its catalogs from lib.rs.

```text
product-operations/src/
  lib.rs
  temperature/
    mod.rs
    read.rs
    set_target.rs
  motion/
    mod.rs
    move_to.rs
    stop.rs
```

### Requirement: Acyclic Product Dependencies

The facade and application runtime SHALL depend on <product>-operations. The operations crate SHALL NOT depend on the facade or application runtime. Its default surface SHALL remain contract-only, with optional runtime dependencies enabled by its runtime feature. Shared pure models and lower-level resource libraries MAY be dependencies where justified.

#### Scenario: Execution Composition

- **WHEN** the application runtime enables operation execution
- **THEN** dependencies follow this direction without a facade/runtime cycle.

```text
product
  -> product-operations
  -> product-runtime [optional]
       -> product-operations [runtime]
            -> product-domain [when needed]
            -> lower-level resource libraries [runtime, when needed]
```

#### Scenario: Contract Consumer

- **WHEN** another plugin depends on operation contracts without enabling runtime in its dependency graph
- **THEN** it can consume messages, definitions and catalogs without optional implementation dependencies.

Cargo feature unification remains an acknowledged limitation; this is not hard isolation between contract and execution builds in a unified graph.

### Requirement: Pure Domain And Runtime Ownership

<product>-domain SHALL retain shared models, transaction declarations, pure rules and required portable service policies/declarations. Resource-owning services, service lifecycle implementations and transaction application requiring live resources SHALL reside in <product>-runtime. Separate transactions or services crates SHALL NOT be mandated solely for symmetry or file count.

#### Scenario: Service Placement

- **WHEN** a service retains device/client resources and implements a live lifecycle
- **THEN** its implementation resides under product-runtime/src/services/<concern>/, while any independently needed portable declarations remain runtime-free.

#### Scenario: Further Extraction

- **WHEN** a service or transaction subsystem becomes large
- **THEN** maintainers assess cohesive ownership, independent reuse and dependency/build isolation before introducing another crate; size alone does not require extraction.

### Requirement: Consistent Skills And Scaffolding

Delivery SHALL update repository-owned Harmos architecture, authoring/testing skills, references, generated copies, scaffolding and documentation. External canonical company architecture/Rust architecture skills and their generated copies are deferred beyond this worktree-only delivery. Superseded unconditional mirrored domain/runtime operation rules SHALL be replaced or explicitly scoped. Installed skill-cache edits SHALL NOT substitute for canonical owner changes.

#### Scenario: New Consumer Guidance

- **WHEN** a consumer follows updated skills or creates a starter project
- **THEN** guidance and templates agree on concern-based operations crates, feature-gated vertical files, pure domain ownership and acyclic dependencies.
