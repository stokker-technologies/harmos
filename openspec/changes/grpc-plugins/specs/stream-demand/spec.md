## ADDED Requirements

### Requirement: Typed Stream Demand in the Calling Scope

Harmos SHALL expose typed live subscriptions on Rust, Python, C# and C++ plugin
contexts. Each row SHALL carry its source timestamp and host receipt timestamp.
Subscriptions SHALL inherit the caller's scope, resources and admission ancestry.
Python subscriptions SHALL support async iteration and explicit `close()` without
requiring a context manager. Delivery SHALL remain bounded and backpressured per
subscriber; a slow subscriber SHALL NOT block its provider or other subscribers.

#### Scenario: Typed Rows Retain Both Timestamps

- **WHEN** a plugin subscribes to a host or another plugin's stream and a row arrives
- **THEN** its declared row type is delivered with source and receipt timestamps
- **AND** interval batches are presented as individual typed rows on their source grid

#### Scenario: Independent Subscriber Delivery

- **WHEN** one subscriber stops requesting rows while a second keeps receiving
- **THEN** only the stopped subscriber's delivery is backpressured
- **AND** provider publication continues and retained memory remains bounded

#### Scenario: Calling Scope Ends

- **WHEN** the subscription's calling scope is cancelled
- **THEN** its subscription ends and releases its demand

#### Scenario: Explicit Close Releases Only This Demand

- **WHEN** one subscriber explicitly closes its subscription
- **THEN** its demand is released while the provider and other subscribers continue

#### Scenario: Admission Receives Inherited Resources

- **WHEN** a plugin subscribes from a call with caller-owned resources
- **THEN** subscription admission observes those resources and the caller's scope
