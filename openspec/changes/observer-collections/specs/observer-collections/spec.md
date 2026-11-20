## ADDED Requirements

### Requirement: One Collection Handler per Due Tick
The host SHALL schedule a declared collection of streams as one demand on the
existing interval-window path for each activation, scope and bound clock. It
SHALL invoke its handler once per due tick while at least one member is listened,
and SHALL NOT invoke it while none is listened. Members SHALL belong to exactly
one collection and share one resolved interval.

#### Scenario: Overlapping Listeners
- **WHEN** two listeners in one scope request different members of a collection
- **THEN** one due tick invokes the handler once and both listeners can receive their member's rows

#### Scenario: Idle Collection
- **WHEN** an activation has an open Samples transport but no listener for any collection member
- **THEN** it receives no collection window and acquires no collection resource

#### Scenario: Distinct Scoped Clocks
- **WHEN** two scopes bind the same collection to independent clocks
- **THEN** advancing one clock invokes only its scope's due ticks and retains that scope on its rows

### Requirement: One Shared Window and Interval
The host SHALL resolve a collection's positive interval from its constant or named
Duration setting using the existing interval resolver. It SHALL select the
shortest resolved window among all declared members once per activation, using
the placement window for a member without an override. Changing listeners SHALL
NOT create a second schedule or change that resolved window.

#### Scenario: Different Member Windows
- **WHEN** members resolve to different window lengths and both have listeners
- **THEN** both use the shortest resolved member window and one handler runs for each due tick

#### Scenario: Batched Controlled Ticks
- **WHEN** one controlled-clock window grants three due ticks
- **THEN** the collection handler runs three times total regardless of the number of listened member streams

### Requirement: Sparse and Repeated Rows Share the Tick Time
A collection invocation SHALL be able to publish zero or more rows for each
member. The host SHALL assign every row of that invocation its tick timestamp,
derived from the granted window, and preserve row order within each stream. A
member with no rows SHALL remain silent without a gap marker or error.

#### Scenario: State, Link and Scan
- **WHEN** one tick publishes one state row, no link row and three scan rows
- **THEN** the state listener receives one row, the link listener receives nothing and the scan listener receives three rows sharing the state row's timestamp

#### Scenario: Entirely Silent Tick
- **WHEN** a successful handler publishes no rows to any member
- **THEN** the tick completes without storing a row or failing its window, including settlement of scoped waits covering that tick

### Requirement: Collection Errors Fail the Listened Window
A handler error SHALL fail the unanswered window for the collection's listened
streams through the current observer WindowFailed behavior. Unanswered-window
output SHALL NOT be exposed as successful rows. Unrelated collections SHALL keep
their own demands. Capacity failures SHALL preserve existing limits and fail-fast
behavior rather than drop rows or grow unbounded staging.

#### Scenario: Handler Error After Publication
- **WHEN** a handler publishes a row and then returns an error before answering its window
- **THEN** the host fails that collection's listened window with the handler cause and exposes none of that unanswered window's rows

#### Scenario: Invalid Member
- **WHEN** a response names a stream outside the collection's declared membership
- **THEN** the host refuses that response and directs the author to publish a declared member

### Requirement: Resources Follow First and Last Listener Demand
The SDK SHALL await on_start once on the transition from no active collection
demand to some demand. It SHALL await on_stop once after the last demand ends and
active handlers settle. Overlapping listeners and scopes SHALL share that resource
lifetime. Idle open transports SHALL NOT keep resources alive. Teardown SHALL use
the existing cancellation and settlement policies.

#### Scenario: Overlapping Listener Lifetimes
- **WHEN** a first listener joins, a second overlaps it, the first leaves and then the second leaves
- **THEN** start runs once before the first tick, stop does not run while the second remains and stop runs once when the final demand ends

#### Scenario: Failed Start
- **WHEN** the first-listener hook fails
- **THEN** the demand fails without invoking the collection handler or retaining an active demand share

#### Scenario: Failure Ends Last Demand
- **WHEN** a handler failure or cancellation ends the last live collection demand
- **THEN** its handler is settled and resource cleanup runs once under the existing settlement policy

#### Scenario: Transport or Process Loss
- **WHEN** the attachment permanently loses its plugin process or transport
- **THEN** the host removes demand ownership within existing settlement bounds without promising execution of an unreachable async hook
