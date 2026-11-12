# Plugin Management Delta

## ADDED Requirements

### Requirement: Client-Facing Plugin Resources

The library SHALL expose `Plugins.Get`, `List`, `Watch`, `Create`, `Set`, `Delete`
and `Restart`, with service-qualified request/response wrappers. `Plugin` SHALL
remain the distinct plugin-implemented carrier service. Reads SHALL accept an
optional issued cut and return the resolved cut. Set SHALL replace the complete
writable value under the expected revision returned by Get.

#### Scenario: Manage a Remote Entry
- **WHEN** an authorized client creates a remote entry, lists it and watches it
- **THEN** the entry becomes ready with its description and catalog digest and an operation can be invoked through the existing host path
- **AND** Set disables and enables it, Restart replaces its activation, and Delete removes the entry without terminating the remote process

#### Scenario: Refuse a Stale Set
- **WHEN** Set supplies a revision superseded by another Set
- **THEN** no writable field changes and the conflict names the action to get again and retry

#### Scenario: Replay Create
- **WHEN** the same Create request identifier and normalized payload are resent, including after host restart
- **THEN** the original response is returned without another entry or process
- **AND** changed input under that request identifier conflicts

### Requirement: Verified Managed Bundles

Managed entries SHALL consume published immutable content from the library upload
mechanism, verify its digest and existing bundle manifest and native executable,
and install under the manager-owned directory before starting anything.

#### Scenario: Manage a Bundled Executable
- **WHEN** a client uploads a valid bundle and creates a managed entry
- **THEN** the entry reports ready and its process identity
- **AND** killing that process reports an unavailable entry with the failure, Restart creates another activation, and Delete awaits process exit and removes the entry

#### Scenario: Refuse Invalid Content Before Startup
- **WHEN** a client supplies a tampered bundle or one without the selected native executable
- **THEN** Create refuses before a process starts, preserves the existing registry and names the failed action

### Requirement: Durable Intent and Shared Hosting

The manager SHALL persist entries and placement through the library's conditional
object interface, reuse existing gRPC attachment and process supervision, retain
unavailable entries, and restore enabled entries when the host starts.

#### Scenario: Restore the Host
- **WHEN** a host with remote, enabled managed and disabled managed entries restarts
- **THEN** their identities, revisions and placement are restored, the remote entry reconnects, the enabled managed entry starts and the disabled entry has no process

#### Scenario: Preserve Authorization and Scope
- **WHEN** a caller invokes manager operations or invokes a plugin through the host
- **THEN** existing visibility and product-supplied grant/authorization policy apply and opaque bound invocation scope is inherited unchanged

#### Scenario: Refuse Unavailable Read Continuation
- **WHEN** a cut or page token is expired, foreign, bound to a different selection or from a previous host instance
- **THEN** the read explicitly refuses instead of silently selecting latest

#### Scenario: Observe Without Silent Gaps
- **WHEN** Watch opens or its bounded event history is exceeded
- **THEN** it supplies a snapshot and resolved boundary or an explicit resynchronization-required event, never an incomplete success
