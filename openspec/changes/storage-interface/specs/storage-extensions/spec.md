## ADDED Requirements

### Requirement: Native Columnar Scans
Storage SHALL expose bounded snapshot-bound Arrow scans with column projection, predicate/time pruning and continuation. Native scans SHALL avoid row-payload reconstruction; unsupported acceleration SHALL use an explicit equivalent bounded fallback or capability error.

#### Scenario: Projected Scan
- **WHEN** a caller pages a projected time range at one retained snapshot
- **THEN** native and fallback results agree and continuation preserves that cut and source positions

### Requirement: Indexed Ordered Seek
Storage SHALL expose indexed search with prefix/range predicates, a deterministic total order and bounded exclusive seek pages. Index visibility SHALL agree with the source cut; incompatible index revisions or expired cuts SHALL refuse continuation explicitly.

#### Scenario: Equal Sort Keys
- **WHEN** matching rows share a sort key and span several pages
- **THEN** the unique tie-breaker returns each row once in stable order at the selected snapshot

### Requirement: Durable Materialized Projections
Storage SHALL register versioned projections and expose bounded reads with a readable source frontier and revision. Durable output SHALL never lag its advertised frontier; replay, rebuild and snapshot invalidation SHALL follow the settled design contract.

#### Scenario: Restart During Advancement
- **WHEN** the backend restarts after interrupted projection advancement
- **THEN** replay preserves results without duplication and the readable frontier names only durable output

### Requirement: Replaceable Time Buckets
Storage SHALL provide TimeBuckets as a query transform preserving first/last/min/max, moments and source positions. Late samples and corrections SHALL replace affected buckets and expose revisions/changed keys while retaining snapshot consistency.

#### Scenario: Late Sample
- **WHEN** a late sample changes a previously materialized bucket
- **THEN** current reads see its replacement and changed key while a retained earlier cut keeps its prior result

### Requirement: Conditional Object Update
Storage SHALL expose expected-version conditional object replacement with an atomic backend check, write and revision/index effects. Conflicts SHALL leave stored data unchanged; accepted mutation replay and durability SHALL retain their distinct guarantees.

#### Scenario: Concurrent Replacements
- **WHEN** two callers replace the same object using its same expected version
- **THEN** exactly one replacement succeeds and the other reports a conflict without overwriting it

### Requirement: Backend Capability Proof
A durable reference backend SHALL implement all five extensions. Every backend SHALL declare support or a specified bounded fallback and preserve existing recovery/file contracts. Native Windows and Linux proof SHALL cover snapshots, replay, invalidation and atomicity.

#### Scenario: Unsupported Facility
- **WHEN** a backend cannot perform a requested extension and has no specified equivalent fallback
- **THEN** it returns an explicit capability error rather than an empty or weaker result

### Requirement: Separate Adoption and Measurement
The extensions SHALL follow the owner-authorized combined delivery sequence: the landed Harmos host surfaces precede storage implementation, which may run beside guest migration; consumer extraction follows its last gRPC wave in its own storage-interface change. Both changes SHALL supply reproducible before/after benchmarks including multi-day data, restart and late samples against agreed regression budgets.

#### Scenario: Consumer Extraction
- **WHEN** Dokime adopts the published extension revision and extracts application persistence
- **THEN** it retains product semantics and named non-persistence owners and compares identical multi-day workloads before and after
