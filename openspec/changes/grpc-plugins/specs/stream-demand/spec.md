## ADDED Requirements

### Requirement: Typed Stream Demand in the Calling Scope

Harmos SHALL expose typed live subscriptions on Rust, Python, C# and C++ plugin
contexts. Each row SHALL carry its source timestamp and host receipt timestamp.
Subscriptions SHALL inherit the caller's scope, resources and admission ancestry.
Python subscriptions SHALL support async iteration and explicit `close()` without
requiring a context manager. Each subscriber SHALL read at its own pace through
the existing bounded live window. Provider publication SHALL never wait on a
subscriber. A subscriber that falls behind the window SHALL receive one explicit
gap item counting the missed rows, then continue from the oldest retained row;
nothing SHALL be dropped silently. Window capacity SHALL come from the existing
stream configuration. Resuming from stored rows is outside this requirement.

#### Scenario: Typed Rows Retain Both Timestamps

- **WHEN** a plugin subscribes to a host or another plugin's stream and a row arrives
- **THEN** its declared row type is delivered with source and receipt timestamps
- **AND** interval batches are presented as individual typed rows on their source grid

#### Scenario: Independent Subscriber Delivery

- **WHEN** one subscriber stops requesting rows while a second keeps receiving
- **THEN** the stopped subscriber retains its own live-window cursor
- **AND** provider publication continues and retained memory remains bounded

#### Scenario: Subscriber Overruns the Live Window

- **WHEN** a subscriber stops reading and publication overwrites rows after its cursor
- **THEN** its next item is one explicit gap counting exactly the overwritten rows
- **AND** subsequent items continue from the oldest retained row
- **AND** provider publication and a subscriber that keeps reading continue independently

#### Scenario: Calling Scope Ends

- **WHEN** the subscription's calling scope is cancelled
- **THEN** its subscription ends and releases its demand

#### Scenario: Explicit Close Releases Only This Demand

- **WHEN** one subscriber explicitly closes its subscription
- **THEN** its demand is released while the provider and other subscribers continue

#### Scenario: Admission Receives Inherited Resources

- **WHEN** a plugin subscribes from a call with caller-owned resources
- **THEN** subscription admission observes those resources and the caller's scope
