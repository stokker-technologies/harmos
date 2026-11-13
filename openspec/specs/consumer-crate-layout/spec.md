# Consumer Crate Layout Specification

## Purpose

Define pure domain and resource-owning runtime responsibilities.

## Requirements

### Requirement: Pure Domain and Runtime Ownership

<product>-domain SHALL retain shared models, transaction declarations, pure rules and required portable service policies/declarations. Resource-owning services, service lifecycle implementations and transaction application requiring live resources SHALL reside in <product>-runtime. Separate transactions or services crates SHALL NOT be mandated solely for symmetry or file count.

#### Scenario: Service Placement

- **WHEN** a service retains device/client resources and implements a live lifecycle
- **THEN** its implementation resides under product-runtime/src/services/<concern>/, while any independently needed portable declarations remain runtime-free.

#### Scenario: Further Extraction

- **WHEN** a service or transaction subsystem becomes large
- **THEN** maintainers assess cohesive ownership, independent reuse and dependency/build isolation before introducing another crate; size alone does not require extraction.
