## Purpose

Define the phases of a plugin activation for native and Wasm placements: admission of frozen declarations, host-invoked initialize and finalize, and the guarantee that finalize releases what initialize acquired.

## ADDED Requirements

### Requirement: Plugin Phase Order

A plugin activation SHALL proceed local register → host admission → initialize → finalize. The host SHALL deliver no invocation or sample window to an activation before initialize succeeds. A plugin SHALL have no start or stop hook. Local registration SHALL freeze declarations before the host's generic checks and product-kind admission.

#### Scenario: Invocation Waits for Initialize

- **WHEN** a client calls a plugin operation while that plugin's initialize is running
- **THEN** the call is admitted only after initialize succeeds, or fails with the activation's initialize failure

### Requirement: Admission Runs No Author Code

Admission SHALL only check the frozen declarations: identities, schemas, references, permissions and limits. It SHALL invoke no handler, prompt, sampling or resource acquisition, and SHALL NOT add a host-invoked registration call beyond reading the declarations.

#### Scenario: Refused Declarations

- **WHEN** a plugin reports declarations with a duplicate operation identity
- **THEN** the host refuses the activation naming the identity, and neither initialize nor finalize is called

### Requirement: Finalize Follows Every Attempted Initialize

Once the host has attempted initialize for an activation, finalize SHALL run exactly once for it, whether initialize succeeded, failed, timed out or was cancelled. Finalize SHALL receive the reason it runs.

#### Scenario: Failed Initialize

- **WHEN** a plugin's initialize returns an error after acquiring part of its resources
- **THEN** the host calls finalize with the initialize-failed reason and then reports the activation failure

#### Scenario: Plugin Without Hooks

- **WHEN** a plugin declares neither initialize nor finalize
- **THEN** both phases complete immediately and its operations behave as before

### Requirement: Cancel Then Finalize

Before stopping or removing a plugin, the host SHALL refuse new invocations to its activation, cancel its in-flight invocations and sample windows, wait for them to settle within their policies and then call finalize.

#### Scenario: Stop During an Invocation

- **WHEN** the host stops while a plugin operation is running
- **THEN** the operation observes cancellation and settles before finalize starts

### Requirement: Finalize After Carrier Loss

When a native plugin's connection closes after initialize was attempted and before finalize, the plugin SDK SHALL run finalize locally with the disconnected reason. A later connection SHALL be a new activation with its own initialize.

#### Scenario: Remote Plugin Loses Its Host

- **WHEN** the connection to a remote plugin drops after its initialize succeeded
- **THEN** the plugin runs finalize with the disconnected reason, keeps running, and initializes again on the next connection

### Requirement: Lifecycle Policies Are Declared

Each plugin SHALL declare the policies of its initialize and finalize phases in its declarations for native and Wasm placements alike. The host SHALL enforce them as the callee's policies.

#### Scenario: Initialize Deadline

- **WHEN** a plugin's initialize exceeds its declared deadline
- **THEN** the host cancels it, calls finalize with the initialize-failed reason and reports the deadline failure
