# Harmos v1 Behaviour

This contract accompanies [plugin.proto](plugin.proto). MUST and MUST NOT are
normative. Each numbered rule has one identically numbered reference-host case.
Cases exercise the fixed fixture plugin below; fixture-specific values are not
requirements on application plugins. Transport is plain HTTP/2 gRPC without TLS
or credentials. The host is always the client.

## Rules

Operations served by a plugin over `harmos.v1` are unary (one request and one response per `Invoke`); request and result streaming remains supported for operations implemented natively in the host process.

1. **Describe and Catalog.** Describe MUST be the first RPC on each connection.
   Peers advertise their highest revision (at least 1); their minimum applies.
   Description MUST identify the plugin, semantic version, SDK and SDK version
   and supply its complete catalog. Catalog message names MUST resolve in types;
   operation IDs MUST be unique. Private operations are callable by their owner
   and host; Public operations by all connected callers. The host MUST refuse
   duplicate providers and incompatible requirements during admission. The fixture
   catalog MUST equal the catalog defined below, including order and hashes.
   Operation identifiers are complete routing keys, with no provider prefix or
   provider selection. Duplicate admission MUST name the identifier and both
   owners, including when one owner is the host. A requirement whose operation
   is absent MAY be admitted; no dependency ordering is required. If its callee
   is present, its fingerprint MUST match. Disconnected or replaced activations
   MUST remove their operations from the registry before replacement admission.
2. **Activation and Started At.** Every connection creates a new host activation
   after Describe and catalog admission. started_at MUST be a valid protobuf
   Timestamp captured once at process startup, unchanged across connections to
   that process. A restarted process captures a new start time. The host exposes
   both activation start times; an old call or handle MUST NOT follow a replacement
   connection. The case reconnects to the same process and compares started_at and
   the catalog; restart detection and host activation bookkeeping belong to host
   validation, not the fixture plugin.
3. **Invoke Result and EOF.** Each Invoke serves exactly one operation attempt.
   Its first HostFrame MUST be Request with a nonempty invocation and attempt >= 1;
   scope is opaque. Payloads MUST be protobuf for the declared type. A successful
   handler MUST send exactly one terminal Response with its payload result set,
   including for an empty encoded message, then close its output. Input half-close
   alone MUST NOT cancel a completed request. conformance.echo returns its input
   unchanged, including empty bytes and binary field values.
4. **Invoke Frame Validation.** Empty input, an unset frame, ChildResponse or
   Cancel before Request, or a repeated Request during an active Invoke MUST end
   that RPC with INVALID_ARGUMENT. Other RPCs on the connection remain usable.
5. **Child Calls and Reordering.** ChildRequest and ChildResponse MUST ride the
   parent Invoke, not another connection. IDs are chosen by the plugin and unique
   within that Invoke. The host routes opaque payloads after visibility, provider
   and fingerprint checks. Replies MUST be matched by ID, even out of order.
   conformance.children sends calls 1 and 2 to conformance.host.echo with Value
   values 01 and 02, respectively, before waiting for replies. It returns a Value
   concatenating reply values in call-ID order. Child calls are unary only.
   The same registry routes host calls and child calls. An absent operation MUST
   fail at call time with a REFUSED failure naming its operation identifier;
   no provider identity is needed. Another plugin's Private operation MUST be
   refused even when its fingerprint matches. A ChildRequest policy overrides
   the callee's policy for that call only; otherwise the callee owns deadlines,
   retries and cancellation grace, enforced using the host clock.
6. **Child Failure Preservation.** A ChildResponse failure is an operation result,
   not a transport fault. conformance.child_failure sends call 1 to
   conformance.host.fail with its input and MUST return every field of the host's
   supplied Failure unchanged, including nested cause, details and diagnostics.
7. **Cancellation and Child Cancel.** Cancel requests cooperative cancellation;
   it does not itself close Invoke. The host may abandon the attempt after its
   cancellation grace. conformance.cancel first sends call 1 to
   conformance.host.pending with its input, then waits. Upon Cancel it MUST send
   ChildCancel(1), then one terminal Failure classified CANCELLED, code cancelled,
   message equal to Cancel.reason, operation conformance.cancel, plugin conformance,
   and a nonempty action. It MUST then close output without waiting for a child
   reply. Connection loss fails outstanding calls; it does not transfer them.
   Connection loss MUST classify those calls as TRANSIENT and MUST NOT fail
   calls on other plugin connections or calls executing on the host. After
   observing ChildCancel the host MUST cancel that child and suppress any
   subsequent ChildResponse for its ID.
8. **Structured Failure.** Application failures MUST use Response.failure,
   preserving classification, stable code, message, action, operation, plugin,
   cause, details and separate support diagnostics. conformance.failure MUST return
   the exact failure fixture below and then EOF, with gRPC OK.
9. **Transport Failure.** Protocol/transport faults MUST use gRPC status rather
   than an application Failure. Unknown operations MUST return NOT_FOUND. Malformed
   protobuf or attempt zero MUST return INVALID_ARGUMENT. No terminal application
   result follows a transport error. Retry policy remains owned by the operation;
   the conformance host does not retry a failed case.
10. **Ticks and Samples.** Publish MUST open with PublishRequest for one stream.
    For an interval stream the host resolves the interval, sends ticks, and the
    plugin MUST answer each tick once, in order, with exactly one sample per
    Samples message at revision 1. Sample.at MUST equal Tick.at exactly; sequence
    MUST count from 1 per call. The conformance.ticks row is Value containing the
    sequence encoded as eight little-endian bytes. A large tick-time jump MUST NOT
    cause synthesized catch-up samples. No wall-clock scheduling is permitted.
11. **Independent Calls.** A stream awaiting a tick MUST NOT block another stream
    or Invoke. gRPC flow control supplies backpressure, with no leases, byte credits
    or publication admission messages. The case opens two idle interval streams,
    completes an echo, then ticks the streams in reverse opening order.
12. **Device Samples.** A stream without an interval is plugin-driven and requires
    no tick. An absent Sample.at means the host stamps arrival using its clock;
    a supplied device timestamp is source time and MUST NOT replace host evidence
    time. conformance.device emits one Value containing UTF-8 device, sequence 1,
    with at absent immediately after open, then awaits demand closure.
13. **Demand and Reopen.** Opening Publish is demand; closing/cancelling it ends
    demand and releases its work. The fixture MUST also close output after input
    EOF. Reopening is a fresh call with sequence 1, independent of previous calls.
    Closing one demand MUST leave the connection usable. Disconnected samples
    are lost; no replay or publication lease is implied.
14. **Publish Frame Validation.** Empty input, unset first frame, Tick before
    open, repeated open, or a tick without a valid Timestamp MUST return
    INVALID_ARGUMENT. An unknown stream MUST return NOT_FOUND. The case checks
    missing tick time; applications MUST also reject invalid timestamp ranges.
15. **Child Correlation Validation.** Unknown or duplicate ChildResponse IDs
    during an active invocation MUST return INVALID_ARGUMENT. IDs MAY be reused
    on another Invoke. The case sends both invalid-ID and duplicate-ID replies
    on separate conformance.children calls.
16. **Stop and Cleanup.** The host MUST call Stop before removing a plugin or
    shutting down, then close after replies or its grace period. The plugin MUST
    complete stop/cleanup hooks before replying. A managed plugin MUST exit
    successfully after the reply; a remote plugin MUST keep serving. The case
    checks the reply; managed mode also checks successful process exit. Remote
    mode calls Describe again after Stop. No active demand remains at this stage.

17. **Schema Fingerprints.** The additional schema fixture below MUST use the
    general Schema Fingerprints rule. The case verifies both advertised hashes
    and retained descriptor metadata, then invokes nested, recursive and projected
    messages, including an optional scalar explicitly set to zero.
18. **Interval Setting.** The additional setting observer below MUST advertise
    its typed section and both interval sources. It MUST answer only host ticks,
    preserving each tick's timestamp and per-call sequence, and close on input EOF.
    Resolution and invalid-setting admission remain host-runtime responsibilities.
    Cases 17, 18 and 19 run before case 16 so managed Stop remains the last RPC.
19. **Built-In Clock Fingerprints.** Every fixture MUST require `harmos.clock.sleep`
    and `harmos.clock.now` with the canonical fingerprints below. The case checks
    advertised requirements, projects their descriptors and compares the host's
    shared clock definitions. `conformance.clock` MUST call now, sleep for 3600
    seconds, then now, using child IDs 1, 2 and 3, with no policy overrides. Return
    the difference in whole seconds as eight little-endian signed bytes in Value.
    The reference host supplies times 100 and 3700 and answers sleep immediately;
    the runtime story instead executes the sleep through its virtual clock.

    | Operation | SHA-256 (Hex) |
    | --- | --- |
    | `harmos.clock.sleep` | `e184a4f9506abaa5a9d9a5dc89d37e42efac5dfed21e1d136037fdf71b079cab` |
    | `harmos.clock.now` | `cd0dbe186ee5a060fac74d88f8ef333444a7899b57da53ecb1fd14d3c92abc81` |

## Host Stream Settings and Evidence

For runtime attachments, `interval_setting` names a fully qualified section field
(`fixture.Settings.interval`) in `Catalog.sections`. The field MUST contain a
positive `google.protobuf.Duration`; an unresolved or invalid setting MUST refuse
admission. It overrides `interval` when both are present.

The runtime stores interval evidence at the tick's host time and device evidence at
host arrival time. A device's supplied timestamp is retained separately as
`context.source_time`, with `seconds` and `nanos` fields. Stored rows use the host's
canonical storage encoding and the schema projected from the admitted protobuf row;
this conversion does not change the protobuf-only transport contract.

## Schema Fingerprints

The following general rule preserves the original minimal Value fixture hashes.
File options do not participate, including in imported well-known types. All
operations, including host operations and required operations, MUST use it. `crates/harmos-protocol/src/schema.rs` is the shared Rust
implementation; other language implementations MUST produce the same bytes.

For each request and response message, independently, project `Catalog.types`
into a data-only FileDescriptorSet as follows:

- Resolve the fully qualified message name (an optional leading dot is ignored).
  Select its enclosing top-level message, including all nested declarations.
- Follow every message field's `type_name` and extension's `extendee`, recursively,
  selecting the enclosing top-level message or enum of each reference. References
  MUST be absolute, with a leading dot. A missing root, unresolved reference,
  duplicate file name or duplicate message/enum name is invalid. Recursive message
  graphs are valid and each enclosing declaration is selected only once.
- Retain only selected top-level messages and enums and their files. Remove file
  services, file-level extensions, file options, source_code_info and each field's json_name,
  including nested fields. This applies equally to imported well-known types:
  retain their file name, package, syntax, selected declarations and reachable imports;
  discard all file options, including language package and code-generation options.
  Retain message, enum, field and oneof options and all other descriptor fields exactly
  as declared; authors using different compilers must agree on those definitions.
- Sort files by name, and top-level messages and enums within each file by name,
  using ascending UTF-8 byte order. Preserve all other repeated declaration order.
  Retain only imports naming retained files, sorted by name with duplicates removed;
  clear public_dependency and weak_dependency indices in this data-only projection.
- Serialize descriptors in numeric field order, omitting default scalar values.

The fingerprint is SHA-256 of the UTF-8 operation ID, one zero byte, the encoded
request projection, then the encoded response projection. No other separator or
length prefix is added. Thus unrelated operations added to a catalog do not
change existing fingerprints. A root nested in a message intentionally includes
its enclosing declaration, so changing that declaration changes the fingerprint.

Catalog digest remains SHA-256 of the entire Catalog with digest empty/omitted;
its repeated declaration order is preserved and its types are not projected.
The host MUST verify the advertised digest and each operation fingerprint before
admitting the catalog. Message names in operations, streams and sections MUST
resolve in its types. This validation does not decode operation payloads.

## Fixed Fixture Catalog

All SDK conformance plugins MUST expose id conformance, version 1.0.0, revision 1,
a nonempty SDK name/version, and a process startup Timestamp. All fixture request,
response and row types are harmos.conformance.v1.Value:

```proto
syntax = "proto3";
package harmos.conformance.v1;
message Value { bytes value = 1; }
```

Catalog.types MUST contain one FileDescriptorProto with name
harmos/conformance/v1/fixture.proto, package harmos.conformance.v1, syntax proto3,
one DescriptorProto named Value, and one FieldDescriptorProto with name value,
number 1, label LABEL_OPTIONAL (1), type TYPE_BYTES (12). All other optional fields
MUST be absent; all other repeated fields empty. This minimal descriptor deliberately
omits source info, json_name and compiler-specific metadata; SDKs normalize it or
construct it directly rather than sending their compiler's complete output.

Catalog.operations MUST contain, in this order, conformance.echo,
conformance.children, conformance.cancel, conformance.failure,
conformance.child_failure. Each uses Value for request/response, Public visibility,
absent policy and empty description. Catalog.requires MUST contain, in order,
conformance.host.echo, conformance.host.pending, conformance.host.fail.

For these fixtures only, fingerprint is SHA-256 of UTF-8 operation ID, one zero
byte, the serialized minimal FileDescriptorSet above for the request, then that
same FileDescriptorSet for the response. Descriptor fields serialize in numeric
field order; repeated order is as declared. Each requirement uses its operation's
fingerprint. This defines reproducible fixture hashes; generic schema graph
canonicalization is outside these cases.

Catalog.streams MUST contain conformance.ticks then conformance.device, each with
row harmos.conformance.v1.Value, empty description and interval_setting. Only ticks
has interval, Duration(seconds=1, nanos=0). Sections are empty. Digest is SHA-256
of Catalog serialized in numeric field order with digest empty/omitted. Default
scalar fields are omitted; repeated entries retain the order above.

The failure fixture has PERMANENT classification, code fixture_failure, message
Fixture Failed, action Use Another Input, operation conformance.failure (or the
operation named by the case), plugin conformance, details {fixture: value}, and
diagnostics Fixture Diagnostics. Its cause has TRANSIENT classification, code
fixture_cause, message Fixture Cause, action Retry, operation conformance.host.fail,
plugin host, absent cause, empty details and diagnostics. The host's child-failure
fixture uses operation conformance.host.fail and is otherwise identical.

## Additional Fixture Catalog

The Fixed Fixture Catalog section defines the original subset and its minimal
Value projection; its original five operations, three requirements and two streams
retain their order and fingerprints. Catalog.types is extended with the following
messages in the same fixture.proto and the imported google/protobuf/duration.proto.
The three SDK fixture.proto copies are identical. The normalized reachable Graph
projection is stored as tools/conformance/fixtures/schema.bin for the
hand-written plugin and Rust SDK fixture; case 17 compares it against each SDK's
advertised graph. It retains message options and omits Duration's file options.

```proto
import "google/protobuf/duration.proto";
message Zebra { Value value = 1; }
message Graph {
  option deprecated = false;
  message Nested {
    optional int32 count = 1;
    Graph next = 2;
  }
  repeated Nested items = 1;
  Zebra tail = 2;
  Graph next = 3;
  google.protobuf.Duration interval = 4;
  Status status = 5;
}
enum Status { STATUS_UNSPECIFIED = 0; STATUS_READY = 1; }
```

Append Public operations with empty descriptions and absent policies, in order:

- conformance.policy: Value to Value. Call conformance.host.echo with the input
  and policy attempts=3, await its reply, then call it again with that reply and
  no policy override. Return the second reply. Child IDs are 1 and 2.
- conformance.schema: Graph to Graph.Nested. Return count equal to items length
  (including explicit zero presence) and next equal to the request.
- conformance.nested: Graph.Nested to Graph. Return next, or an empty Graph.
- conformance.projected: Graph to Value. Return tail.value, or an empty Value.

Append `conformance.clock` (Value to Value, Public, absent policy and empty
description) after these operations. Append requirements `harmos.clock.sleep`
then `harmos.clock.now` after `conformance.host.schema`. Include their reachable
clock types from `harmos/v1/plugin.proto`, Duration and Timestamp in Catalog.types.

Append conformance.settings after the original streams, with row Value, default
interval one second and interval_setting harmos.conformance.v1.Graph.interval.
Its rows follow conformance.ticks. The single section has type
harmos.conformance.v1.Graph and payload Graph(interval=Duration(seconds=2)).
It overrides the default. Append requirement conformance.host.schema, using the Graph-to-Value fingerprint.
Catalog digest
covers this complete extended inventory; only fingerprint projections discard
unrelated declarations. Case 5 also checks the policy fixture; case 14 checks
invalid timestamp ranges as well as missing timestamps.

Rules 1, 5 and 7 also constrain host registry admission, opaque routing, visibility,
clock-driven policy enforcement, activation replacement and connection-loss
isolation. A fixture plugin cannot observe those host internals. Their proof lives
in host-runtime validation; the reference-host cases check the plugin-visible
frames, catalog and per-call policy preservation without reimplementing that runtime.

## Running the Reference Host

```sh
mise run prepare
mise run build -- --package harmos-conformance
# Append .exe on Windows.
target/debug/harmos-conformance --plugin target/debug/harmos-conformance-plugin
target/debug/harmos-conformance --plugin /path/to/sdk-plugin -- argument
target/debug/harmos-conformance --address 127.0.0.1:53124
mise run test -- --package harmos-conformance --target integration
```

Managed mode starts the executable, drains stdout until the line
harmos listening 127.0.0.1:PORT (a bound loopback socket), then dials it.
HARMOS_ADDRESS overrides the dial address after readiness. Plugins MUST honor
HARMOS_ADDRESS as their bind address when set. Other stdout lines are ignored;
SDK logs should use stderr. The host drains remaining stdout and owns process
cleanup on failures. Executable arguments follow --. Remote mode accepts a socket
address or http:// URL, owns no process, and never kills the endpoint.

The Rust fixture defaults to managed lifecycle; pass --remote to keep it running
after Stop. HARMOS_ADDRESS selects its listener in either mode. The host prints
one pass/fail table row per rule and exits 0 only if every case passes; exit 1 is
case failure and exit 2 is setup/CLI failure. Each case and managed startup/shutdown
has a 30-second transport hang guard. These guards do not advance scenario time.
Cases use literal host tick timestamps and protocol events, never sleeps, elapsed
time assertions, Tokio's paused transport clock, or a local SDK clock. Only the
reference plugin's process startup timestamp reads the system clock once.

## Host Operation Messages

Built-in host operations use the generated messages in `host.proto` and `storage.proto`.
The handler owns its deadline, retry and visibility policy; consumers configure admission,
concurrency and response sizes. Native streaming handlers remain in process and expose the
same client-facing gRPC streaming/control envelopes.

An assignment names an owner-qualified operation, contains exactly 32 fingerprint bytes and
a nonzero provider generation. Local operation identifiers must be unique within a catalog.
The first valid catalog freezes an activation's assignments; reinstalling identical entries
is permitted, and changing an entry or its order is refused. This catalog is protobuf even
inside `harmos.v1.GuestInitialize`.

File SHA-256 values contain exactly 32 bytes. A finished file reference has a positive stored
position, and its length/hash must agree with any expected integrity metadata. Local file
paths cross the protocol as UTF-8; native paths that cannot be represented are refused.
Wide query integers use minimal decimal strings, datetimes use signed nanoseconds since the
Unix epoch, exact fractions are reduced, and floating-point query values must be finite.
Directory response limits count the actual generated protobuf entry/event size.

The WebAssembly boundary uses `harmos.v1.GuestCommand`, `GuestChild`, `Response`
and `Failure`. Submit and child control require the original finite attempt
capability; retained generation capabilities permit declared calls and publication
only while that generation remains active. Guest operations are unary.
