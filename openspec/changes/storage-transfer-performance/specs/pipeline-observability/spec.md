## ADDED Requirements

### Requirement: New Pipeline Instrumentation

New storage, retained-read, filesystem, JSON backend and carrier paths SHALL follow the existing execution observability contract. Spans SHALL identify bounded operation or batch stages without per-element spans, payload contents, raw paths or eager diagnostic formatting. Captured request context SHALL follow asynchronous and blocking work through actual completion and cancellation without changing ownership or admission.

#### Scenario: Retained Read Is Projected

- **WHEN** a caller creates a read session and consumes a retained result through projection
- **THEN** the operation and admission boundaries are observable while the same memory lifetime and refusal behavior remain in force

#### Scenario: Filesystem Work Continues Across A Handoff

- **WHEN** a new filesystem or JSON operation hands work to a task or blocking worker
- **THEN** the worker retains its originating subscriber and records its actual work lifetime even if the waiting caller is cancelled

#### Scenario: Carrier Prepares A Transfer

- **WHEN** a new carrier path prepares, admits or frames a transfer
- **THEN** its stages are observable without extra payload encoding or copying for diagnostics

#### Scenario: Tracing Is Disabled

- **WHEN** the relevant target or level is disabled
- **THEN** results, memory accounting, cancellation and durable publication remain unchanged without per-item diagnostic work