## ADDED Requirements

### Requirement: Plugins Declare Lanes

A plugin description SHALL be able to list lanes, each with a name and a
capacity that is serial, a fixed concurrent width or unbounded, and each
operation SHALL be able to name one declared lane. An operation that names none
SHALL belong to the plugin's default unbounded lane.

#### Scenario: Operation in a Serial Lane

- **WHEN** a plugin declares a serial lane and assigns two operations to it
- **THEN** its description lists the lane and both operations name it

#### Scenario: Plugin Without Lanes

- **WHEN** a plugin declares no lanes
- **THEN** its description and behavior are unchanged

### Requirement: Lane Declarations Are Validated at Admission

The host SHALL refuse a plugin whose lane declarations are invalid, with an
actionable error naming the lane: an empty or reserved name, a name declared
twice, a zero capacity, or an operation naming an undeclared lane.

#### Scenario: Undeclared Lane

- **WHEN** an operation names a lane the plugin does not declare
- **THEN** admission fails and names the operation and the lane

### Requirement: Lanes Are Scoped to the Plugin Instance

The host SHALL create a plugin's lanes for that plugin instance only. Two
plugins, or two instances of one plugin, that declare a lane with the same name
SHALL NOT share its capacity, and a plugin lane SHALL NOT collide with an
application lane of the same name.

#### Scenario: Same Name in Two Plugins

- **WHEN** two plugins each declare a serial lane with the same name and both are invoked
- **THEN** each plugin's invocations are serialised independently

### Requirement: Invocations Are Admitted Through Their Lane

The host SHALL admit every invocation of a plugin operation through that
operation's lane with the existing lane admission. Invocations beyond the
lane's capacity SHALL wait in arrival order and remain cancellable while they
wait.

#### Scenario: Serial Lane Queues

- **WHEN** a second invocation arrives for a serial lane while one is running
- **THEN** it starts only after the first settles

#### Scenario: Different Lanes Do Not Block

- **WHEN** an invocation in a serial lane is running and one arrives for another lane
- **THEN** the second starts without waiting

#### Scenario: Cancelled While Waiting

- **WHEN** a waiting invocation is cancelled
- **THEN** it ends as cancelled and never reaches the plugin

### Requirement: Lanes Are Released With the Plugin

The host SHALL release a plugin's lanes when that plugin instance stops or is
replaced. Invocations still waiting SHALL end with the existing not-available
failure.

#### Scenario: Plugin Stops With Waiters

- **WHEN** a plugin stops while invocations wait in one of its lanes
- **THEN** each waiting invocation fails as not available and the lane no longer exists
