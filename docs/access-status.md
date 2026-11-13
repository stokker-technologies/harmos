# Access and Host Status

Host applications configure access through `runtime.grpc().configure_access` and
status through `runtime.services().configure_status`. These register library-owned
client operations on the same runtime endpoint. A host supplies its identity,
credential store, transport policy and collection budgets; it retains its own
authorization policy and application services.

## Access

`harmos.v1.access.Listener.Get/Set` reads and replaces the complete listener value:
enabled, socket endpoint and TLS certificate names. Pass the revision returned by
Get to Set. A stale revision refuses with “get again and retry”. The reported bound
endpoint includes the actual port when the configured socket uses port zero.
Access always uses TLS and bearer authentication. Successful authentication supplies
`grpc::access::Principal { pairing_id }` in the call context before application
middleware and operation authorization.

The host implements `grpc::access::CredentialStore`:

```rust
fn load(&self) -> Pending<'_, Option<Credentials>>;
fn commit(&self, expected_revision: u64, value: Credentials) -> Pending<'_, ()>;
```

Commit atomically compares the revision and replaces all settings, key material,
certificate and pairing hashes. Expected revision zero requires absence. A failed
commit must leave every byte unchanged. Protect private keys using the application's
chosen secret store. Harmos chooses neither an operating-system store nor a layout.
The manager is the sole writer of this value; external revision changes require
reloading access configuration.

`Pairings.Create` returns an endpoint, pinned certificate and one bearer credential.
Retain that credential securely: only its hash is persisted and it is never returned
by List. Durable independent pairings can be listed in one complete response and revoked
with Delete. Pairing positions and tokens expire when access changes or the process
restarts; an expired selection fails explicitly.

`Certificates.Create` regenerates and publishes a certificate through the existing
reloadable TLS resolver. Existing TLS connections survive, all pairing credentials
are revoked, and new connections must trust the new certificate. Create and distribute
fresh pairings after rotation. Changing certificate names through Listener.Set also
rotates trust. Keys are validated before storage or publication. Structured store failures name
the attempted action in the shared Failure action field and preserve the live configuration.

An application can retain a separately authorized management listener for bootstrap
and recovery while access is disabled. Pairing authentication applies to the dedicated
access listener. Management-listener authorization remains application policy.
The gRPC manager retains and shuts down the access attachment.

## Status

`harmos.v1.Status.Get` returns the supplied application name/version and stable host
identifier, library version, runtime start time, availability and named health
components. Status.Watch opens with the current status and follows changes; slow
observers receive the latest coalesced status. Host time comes from the runtime clock.

Register a `services::status::Contribution` through `services.status()?.contribute`.
The trait supplies health components and text diagnostics entries. Retain its
registration for the contribution's lifetime, call `registration.changed()` after
changing its facts, and drop it to withdraw them. Component names must be unique.
Provider failure appears as unavailable health; diagnostics collection fails with
the action named. A contributor never replaces the library's collection service.

Logs and TraceEvents expose List/Watch in `harmos.v1.status`. Supply positive row
and byte bounds. List returns `resolved_at` and an optional continuation token;
continue at that exact boundary even while new events arrive. Watch sends an empty
opening boundary and then bounded batches strictly after its position. It refuses
retention gaps and foreign, unissued or expired positions. Histories are volatile
process diagnostics and do not claim storage durability.

Use `status.layer()` with the application's tracing subscriber, or explicitly
record log/trace events. Register sensitive values with `status.redaction.register`
before recording or collecting them. Sensitive field names redact automatically.
Redaction occurs before retention, watch delivery and archive construction.
Contributions supply text so archive contents can follow the same redaction policy.

`Diagnostics.Get` freezes a bounded tar archive when its name is absent and offset
is zero. It includes host facts, log/trace tails and contributed entries. The reply
returns the content name, total length and SHA-256 digest. Repeat Get with that name,
an offset and a configured maximum range size; verify the completed content digest.
Collection retains one archive; another collection replaces it, and old names fail
explicitly. Unsafe paths, duplicate entries and oversized archives fail instead of
producing partial archives. Diagnostics and stored-file reads share
`harmos_storage::get_content_bytes`, including range bounds, short-read handling
and full-content digest verification. The service manager retains archive lifetime.

Rust clients use generated contracts under `harmos::client::v1` and its `access`/`status` modules.
The mechanical root exposes `client.access.listener`, `client.access.pairings`,
`client.access.certificates` and `client.status` with its nested resources. Host applications delete their duplicate listener,
pairing/rotation handlers, generic status services and collection machinery after
composing these services. Keep application identity, fact contributors, authorization,
settings persistence and secret-store integration in the application.
