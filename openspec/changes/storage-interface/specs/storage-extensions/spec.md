## ADDED Requirements

### Requirement: Mutation Window Lifetime
Issuance windows SHALL rotate at half their configured lifetime and retain at most two live windows. New mutations SHALL use their own window's deadline; owned completion SHALL survive expiry. Receipt eviction SHALL preserve issuance windows and recover durable outcomes through keyed backend lookup. Expired or unknown identities without a durable receipt SHALL return a distinct, actionable, transient mutation-expiry kind requiring a fresh identity.

#### Scenario: Rotation During Reservation
- **WHEN** a mutation reserves just before half-life and admission continues after rotation but before its own deadline
- **THEN** admission and completion succeed using the original window and expiry

#### Scenario: Expired Identity and Bounded Windows
- **WHEN** issuance continues for many lifetimes and an identity passes its own deadline
- **THEN** at most two issuance windows remain live and the identity receives `BackendFailure::MutationExpired` through the existing error carrier, with native `storage.kind = STORAGE_FAILURE_MUTATION_EXPIRED` and instructions to obtain a current window and issue a fresh identity

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
Harmos SHALL supply the backend contract, memory reference and importable conformance kit for all five extensions. A product SHALL own its durable backend, including SQLite, and prove durable restart there. Every backend SHALL declare support or a specified bounded fallback and preserve recovery/file contracts. Native Windows and Linux proof SHALL cover snapshots, replay, invalidation and atomicity.

#### Scenario: Unsupported Facility
- **WHEN** a backend cannot perform a requested extension and has no specified equivalent fallback
- **THEN** it returns an explicit capability error rather than an empty or weaker result

### Requirement: Separate Adoption and Measurement
The landed extensions SHALL be adopted through the typed native operation boundary. Product extraction and durable-backend proof SHALL remain in the product change. Library measurements SHALL use backend-neutral/in-memory workloads; product measurements SHALL cover durable restart, multi-day data and late samples. Proposed numerical budgets SHALL NOT become release gates without owner acceptance.

#### Scenario: Consumer Extraction
- **WHEN** a product adopts the published extension revision and extracts application persistence
- **THEN** it retains product semantics and named non-persistence owners and compares identical multi-day workloads before and after

### Requirement: Exact-Scope Unfinished File Production Discovery
Storage SHALL expose Private native `ListFileProductions` with exact scope, optional PageRequest and ReadCut. Responses SHALL provide staged session identity, writer generation, accepted and durable byte lengths, resolved_at and next_page_token. Discovery SHALL reuse the staging set populated by `recover_files` and exclude published files, private transfer staging and other scopes.

#### Scenario: Exact Scope and Unfinished Membership
- **WHEN** unfinished and published files exist in one scope and an adjacent child scope
- **THEN** discovery returns only unfinished productions whose scope equals the requested identity

### Requirement: Bounded File Production Pages
The operation SHALL bound page rows, retained response bytes and cursor input by storage response policy. A first entry exceeding the byte bound SHALL refuse with an actionable error rather than return an empty nonterminal page.

#### Scenario: Response Budget
- **WHEN** the requested row or byte limit is smaller than the available unfinished set
- **THEN** the operation returns a bounded page and continuation or refuses if the first entry cannot fit

### Requirement: Restart-Resumable File Discovery
Continuations SHALL bind exact scope, store incarnation, saved storage position and exclusive file identity and survive restart of that history. Membership and progress SHALL be live rather than historical staging snapshots. A product SHALL fence admission before traversal and settlement; newly admitted identities before a consumed key require fresh traversal.

#### Scenario: Bounded Resumable Traversal
- **WHEN** a fenced scope contains more productions than one page and storage restarts between pages
- **THEN** the issued cursor resumes in identity order without repeating earlier identities and retains its selected storage boundary

#### Scenario: Lost Create Response and Recovered Progress
- **WHEN** creation commits but its caller loses the response and storage restarts
- **THEN** startup recovery makes the production discoverable with its current writer generation and durable byte prefix

#### Scenario: Mismatched Continuation
- **WHEN** a continuation is malformed or supplied with a changed scope, foreign history or conflicting cut
- **THEN** discovery refuses explicitly and names how to restart the traversal

### Requirement: File Discovery Boundary and Settlement
Shared discovery protocol messages SHALL project into every generated SDK without exposing a storage wire service or changing native caller admission. Native results SHALL supply the session required by existing publish and abandon operations.

#### Scenario: Settle Before Scope Deletion
- **WHEN** DeleteScope refuses because unfinished productions remain and the consumer fences admission, discovers them, claims recovered writers through the existing CreateFileWriter operation, then publishes or abandons them with existing operations
- **THEN** fresh discovery is empty and DeleteScope succeeds
