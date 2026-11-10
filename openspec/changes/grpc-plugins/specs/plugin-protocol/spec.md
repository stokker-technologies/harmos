## ADDED Requirements

### Requirement: One Sidecar Protocol
gRPC with the `harmos.v1` package SHALL be the only sidecar protocol, defined by checked-in
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
- **WHEN** the host starts a managed plugin
- **THEN** the plugin announces the loopback address it listens on and the host connects to it

#### Scenario: Remote Plugin
- **WHEN** the product supplies a URL whose server is reachable
- **THEN** the host connects, admits its catalog and routes operations to it

#### Scenario: Child Call
- **WHEN** an operation running in a plugin calls a required operation
- **THEN** the request and its answer travel on that operation's `Invoke` call and the plugin
  opens no connection of its own

### Requirement: Describe on Every Connect
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
The host SHALL open one `Publish` call per declared stream it wants, and the plugin SHALL send
one sample per message on it.

#### Scenario: Demand Starts and Ends
- **WHEN** the host opens and later closes a stream's call
- **THEN** the plugin produces samples for that stream only between the two

#### Scenario: Observer
- **WHEN** an observer is declared with an interval on a stream whose call is open
- **THEN** the host sends one tick per interval and the plugin answers each with one sample
  carrying the tick's time

#### Scenario: Independent Streams
- **WHEN** one stream's consumer is slow
- **THEN** other streams and operation calls keep flowing

### Requirement: Shared Authoring Surface
A plugin that reaches the outside world only through its context SHALL build as a sidecar or a
guest from the same source.

#### Scenario: Portable Plugin
- **WHEN** a plugin using only operations, child calls, resources and settings is built for a
  native target and for Wasm
- **THEN** both builds succeed without source changes

#### Scenario: Native Facility
- **WHEN** a plugin that declares a stream or an observer is built as a guest
- **THEN** the build fails with a message naming the unsupported facility

### Requirement: Orderly Stop
The host SHALL call `Stop` on every connected plugin before it shuts down, and SHALL close only
after each has replied or the grace period has ended.

#### Scenario: Host Shuts Down
- **WHEN** the host shuts down with plugins connected
- **THEN** each plugin runs its stop and cleanup hooks before the host closes

#### Scenario: Plugin Does Not Reply
- **WHEN** a plugin has not replied when the grace period ends
- **THEN** the host closes anyway and records which plugin did not reply

### Requirement: Hand-Written Plugins
A server implementing `harmos.v1` from the `.proto` files without an SDK SHALL be a valid plugin.

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

### Requirement: Two Visibility Levels
An operation SHALL be `Private`, callable by its declaring plugin and the host, or `Public`,
callable by everyone connected.

#### Scenario: Private Operation
- **WHEN** another plugin or a client calls a `Private` operation
- **THEN** the call is refused
