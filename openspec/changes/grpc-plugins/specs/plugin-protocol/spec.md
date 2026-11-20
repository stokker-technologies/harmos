## ADDED Requirements

### Requirement: Durable Entry-Scoped Plugin State
The host SHALL persist one opaque revisioned value per plugin entry through its
existing StateManager journal and complete snapshots, independently of the
application's compile-time state. Get and Put SHALL admit only the trusted calling
entry. Put SHALL compare the expected revision, enforce its configured operation
size limit, and return only after durable journal flush. Empty bytes SHALL clear
the value while advancing the revision. Removing an entry SHALL retire its state.

#### Scenario: Initialize After Host Restart
- **WHEN** a plugin with outstanding effects initializes after a host restart
- **THEN** Get returns its last durable value and revision even with application state `()`

#### Scenario: Conflicting Update
- **WHEN** Put carries an outdated expected revision
- **THEN** the host returns `state_revision_conflict` without changing the value

#### Scenario: Durable Intent
- **WHEN** recovery flush has not completed
- **THEN** Put has not returned success and the plugin cannot treat intent as durable

### Requirement: Bounded Recovery Retention
After a complete snapshot is durable, the storage manager SHALL invoke backend
compaction to retain the newest complete snapshot and its uncovered journal suffix.

#### Scenario: Repeated Snapshots
- **WHEN** multiple complete snapshot cycles cover successive writes
- **THEN** covered entries and superseded snapshots do not accumulate

### Requirement: Canonical Plugin Service Identity
Every SDK plugin SHALL register its canonical gRPC service identity independently of the generated source module layout.

#### Scenario: Conformance Calls the Plugin by Its Canonical Path
- **WHEN** the conformance host calls `/harmos.plugin.v1.Instance/Get` directly
- **THEN** every SDK plugin returns its declarations

### Requirement: One Wrong-Caller Failure
Wrong-caller admission SHALL return classification REFUSED, code `-32604`, action `Call` and the refused operation across gRPC, sidecar child replies, samples, guest imports and nested invocations. The message SHALL name the action and operation.

#### Scenario: An Invocation Forwards a Denied Child Call
- **WHEN** a client invocation runs an operation whose child call has the wrong caller
- **THEN** the original refusal kind, code, action and operation survive nesting without INTERNAL or PERMANENT reclassification

#### Scenario: An Encoded Invocation Forwards an Explicit Native Status
- **WHEN** a native operation returns an explicit gRPC status through a product-owned invocation wrapper
- **THEN** the direct and encoded calls retain the same status kind, action, structured details and cause, including Unimplemented

#### Scenario: A Guest Calls a Refused Product Operation
- **WHEN** a guest import calls a product wrapper that does not admit plugin callers
- **THEN** the refusal retains the action and operation and the guest remains usable

### Requirement: Protocol-Owned Contracts
Proto files SHALL own wire contracts; protocol SHALL contain generated definitions and light helpers only. Behaviour needed in another language SHALL be specified in words and implemented in each SDK.

#### Scenario: Shared Behaviour
- **WHEN** an SDK needs a behaviour exposed by a Rust protocol helper
- **THEN** its implementation follows the written language-neutral contract

### Requirement: Reduced Host Foundations
Harmos domain SHALL retain only non-wire native state/recovery, encoding and storage contracts. Operations SHALL remove their runtime feature once no contract-only consumer remains, retaining handlers beside declarations.

#### Scenario: Final Boundary Migration
- **WHEN** contract-only consumers have migrated to protocol or SDK
- **THEN** operations has no runtime feature and domain contains no duplicate wire models

### Requirement: One Sidecar Protocol
gRPC with shared model and plugin carrier definitions SHALL be the only sidecar protocol, defined by checked-in
`.proto` files that need no Cargo to consume.

#### Scenario: Old Wire Removed
- **WHEN** the migration completes
- **THEN** the framed stdin/stdout wire, its handshake, credits and adapters no longer exist

#### Scenario: Definition Drift
- **WHEN** Rust definitions change without updating the checked-in `.proto` files
- **THEN** a repository check fails

### Requirement: Host Dials the Plugin
The host SHALL connect to the plugin for both placements, and plugin-to-host operation calls
SHALL travel on the `Invoke` call that started them.

#### Scenario: Managed Plugin
- **WHEN** the host starts a managed plugin with `HARMOS_HOST_ENDPOINT` and `HARMOS_LAUNCH_TOKEN`
- **THEN** the plugin calls `harmos.plugin.v1.Launches/Announce` with its token and the loopback
  address it listens on, and the host connects to it

#### Scenario: Managed Launch Credential
- **WHEN** an announcement carries a wrong, spent or revoked token
- **THEN** the host refuses it with `UNAUTHENTICATED` and no pending launch receives an address

#### Scenario: Managed Plugin Exits Before Announcing
- **WHEN** a managed process exits before announcing
- **THEN** the load fails with the executable, process identifier, exit status and the bounded
  tail of its standard error, and the token is revoked

#### Scenario: Remote Plugin
- **WHEN** the product supplies a URL whose server is reachable
- **THEN** the host connects, admits its catalog and routes operations to it

#### Scenario: Child Call
- **WHEN** an operation running in a plugin calls a required operation
- **THEN** the request and its answer travel on that operation's `Invoke` call and the plugin
  opens no connection of its own

### Requirement: Admission on Every Connect
The plugin SHALL report its id, version, start time and catalog with a content digest, and each
connect SHALL create a new activation.

#### Scenario: Same Process Reconnects
- **WHEN** the connection drops and the host reconnects to the same process
- **THEN** the new activation reports the same start time as the previous one

#### Scenario: Process Restarted
- **WHEN** the plugin process restarts and the host reconnects
- **THEN** the new activation reports a different start time

#### Scenario: Call in Flight
- **WHEN** the connection drops during an operation
- **THEN** that attempt fails and the operation's policy decides whether it is retried

### Requirement: Typed Calls Between Plugins
A plugin SHALL call another plugin's Public operation through the host with a protobuf payload
generated from the provider's definitions, whatever language either is written in.

#### Scenario: Cross-Language Call
- **WHEN** a plugin in one language calls a Public operation served by a plugin in another
- **THEN** the provider receives the request and the caller receives its typed response

#### Scenario: Definition Mismatch
- **WHEN** a caller was built against a different definition than the provider serves
- **THEN** the host reports the mismatch naming both plugins and refuses the call

#### Scenario: Provider Absent
- **WHEN** a plugin calls an operation whose provider is not connected
- **THEN** the call is refused naming the missing plugin, and succeeds once that plugin connects

### Requirement: Independent Plugins
The host SHALL connect and disconnect each plugin without ordering it against other plugins.

#### Scenario: Provider Connects Later
- **WHEN** a plugin connects before a plugin whose operations it requires
- **THEN** it becomes ready without waiting

#### Scenario: Duplicate Identifier
- **WHEN** a plugin declares an operation identifier that a connected plugin or the host already serves
- **THEN** the plugin is refused at connect with a reason naming the identifier and its current owner

### Requirement: Host-Opened Streams
The host SHALL open Samples demand for selected streams. Interval producers SHALL answer
host-granted windows with columnar parts; device-driven producers SHALL preserve source time.
Closing demand SHALL settle accepted ingestion and producer cleanup.

#### Scenario: Demand Starts and Ends
- **WHEN** the host opens and later closes a stream's call
- **THEN** the plugin produces samples for that stream only between the two

#### Scenario: Observer
- **WHEN** an observer is declared with an interval on a stream whose call is open
- **THEN** the host grants windows of samples due on the interval's grid and the plugin answers
  each window with columnar batches, split at its maximum message size

#### Scenario: Independent Streams
- **WHEN** one stream's consumer is slow
- **THEN** other streams and operation calls keep flowing

### Requirement: Hand-Written Plugins
A server implementing the Harmos carrier and its product contract directly from their
protocol definitions SHALL be eligible for admission without using an author SDK.

#### Scenario: No SDK
- **WHEN** the host connects to a server written directly against the generated stubs
- **THEN** it admits the catalog and calls its operations as for any other plugin

### Requirement: Host-Owned Clock
Every wait an SDK offers SHALL be served by the host's clock, and the host SHALL be able to run
that clock virtually in a test.

#### Scenario: Wait in a Plugin
- **WHEN** an operation in a plugin sleeps through its context
- **THEN** the call returns when the host's clock has advanced by that duration

#### Scenario: Accelerated Story
- **WHEN** the host runs a virtual clock and every call in flight is waiting on it
- **THEN** the clock jumps to the next timer and the waiting call continues without real time
  passing

#### Scenario: Concurrent Scoped Units on One Activation
- **WHEN** a product binds two trusted scopes and independent clocks to an already admitted activation
- **THEN** each scope's Samples selection and descendant Time operations use its clock on the same physical connection
- **AND** accepted ingestion blocks that clock, closing one scope awaits ingestion and producer cleanup, and the other scope and unscoped demand remain usable
