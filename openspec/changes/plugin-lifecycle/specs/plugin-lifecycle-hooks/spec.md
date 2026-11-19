## Purpose

Define the phases of a plugin activation for native and Wasm placements: admission of frozen declarations, host-invoked initialize and finalize, and the guarantee that finalize releases what initialize acquired.

## ADDED Requirements

### Requirement: Plugin Phase Order

A plugin activation SHALL proceed local register → host admission → initialize → finalize. The host SHALL deliver no invocation or sample window to an activation before initialize succeeds. A plugin SHALL have no start or stop hook. Local registration SHALL freeze declarations before the host's generic checks and product-kind admission.

#### Scenario: Invocation Waits for Initialize

- **WHEN** a client calls a plugin operation while that plugin's initialize is running
- **THEN** the call fails with the existing not-ready refusal until initialize succeeds

### Requirement: Admission Runs No Author Code

Admission SHALL only check the frozen declarations: identities, schemas, references, permissions and limits. It SHALL invoke no handler, prompt, sampling or resource acquisition, and SHALL NOT add a host-invoked registration call beyond reading the declarations.

#### Scenario: Refused Declarations

- **WHEN** a plugin reports declarations with a duplicate operation identity
- **THEN** the host refuses the activation naming the identity, and neither initialize nor finalize is called

### Requirement: Finalize Follows Every Attempted Initialize

Once the host has attempted initialize for an activation, finalize SHALL run exactly once for it, whether initialize succeeded, failed, timed out or was cancelled. Native hooks SHALL use reserved `<plugin>.initialize` and `<plugin>.finalize` operations with Empty request and response messages; no reason payload or new RPC is added. Each declared hook uses existing operation policy/defaults and is callable only by the host lifecycle.

#### Scenario: Failed Initialize

- **WHEN** a plugin's initialize returns an error after acquiring part of its resources
- **THEN** the host calls finalize through Invoke with an admitted context and then reports the activation failure

#### Scenario: Plugin Without Hooks

- **WHEN** a plugin declares neither initialize nor finalize
- **THEN** both phases complete immediately and its operations behave as before

### Requirement: Cancel Then Finalize

Before stopping or removing a plugin, the host SHALL refuse new invocations to its activation, cancel its in-flight invocations and sample windows, wait for them to settle within their policies and then call finalize.

#### Scenario: Stop During an Invocation

- **WHEN** the host stops while a plugin operation is running
- **THEN** the operation observes cancellation and settles before finalize starts

### Requirement: Lifecycle Visibility

The SDK SHALL reserve the two lifecycle identifiers against ordinary operation declarations. The host SHALL exclude them from ordinary routing for clients, plugins and host callers.

#### Scenario: Client Attempts a Lifecycle Call

- **WHEN** a client invokes either lifecycle identifier with the correct fingerprint
- **THEN** the host refuses it without dispatching the hook

### Requirement: Subscription Lifetime Is Explicit

The SDK SHALL NOT present invocation-owned or window-owned child Watch calls as plugin-lifetime subscriptions. The unchanged wire has no activation-owned subscription context.

#### Scenario: Initialize Returns With a Subscription

- **WHEN** initialize opened a child Watch and then returns
- **THEN** its invocation ends and the Watch cannot be retained until finalize

### Requirement: Lifecycle Policies Are Declared

Native hook operations MAY declare existing operation policies; absent policies SHALL use the existing operation default. The host SHALL enforce them as ordinary callee policies without a new lifecycle setting.

#### Scenario: Initialize Deadline

- **WHEN** a plugin's initialize exceeds its declared deadline
- **THEN** the host cancels it, calls finalize through Invoke with an admitted context and reports the deadline failure

#### Scenario: Cancelled Initialize

- **WHEN** initialization is cancelled after it started
- **THEN** the host settles its invocation and invokes finalize exactly once before Stop

#### Scenario: Repeated Stop

- **WHEN** stop is requested twice for one activation
- **THEN** finalize is invoked only once and Stop still follows a failed finalize
