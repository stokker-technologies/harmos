# Plugin Management

`harmos_runtime::plugin::manager` owns a persistent set of hosted plugins.
Its client-facing `harmos.host.v1.Plugins` service is separate from
`harmos.v1.Plugin`, which each plugin executable implements.

## Host Composition

Register the manager's object catalog with the storage backend and storage
manager. Supply an exclusive manager namespace, installation directory, native
bundle target and every capacity in `PluginManagerSettings`. Construct the
manager with the storage handle and `PublishedContent`, register it with
`setup.settings().plugins(manager.clone())`, launch the runtime in an `Arc`, then
call `manager.start(&runtime).await`. Storage must already be started; startup
fails explicitly if its conditional object primitive is unavailable.

```rust
use std::sync::Arc;
use harmos_runtime::plugin::manager::{Manager, PublishedContent};

// Include plugin::manager::catalog() in the storage and backend object catalogs.
let manager = Manager::new(storage.clone(), Arc::new(PublishedContent(storage.clone())), settings)?;
// In the runtime's existing setup closure:
setup.settings().storage(&storage).plugins(manager.clone());
// After the configured Runtime has launched:
let runtime = Arc::new(runtime);
manager.start(&runtime).await?;
```

The host's existing gRPC listener projects the registered operations. Generated
Rust clients use `harmos_protocol::host::plugins_client::PluginsClient`;
`harmos::client::Client` can bind the same generated request/response contracts.
Rust SDK `OperationRequest` bindings exist for each unary manager operation.
Native hosts can also use the `Management` trait on `Manager` directly.

`PluginManagerSettings` supplies entry and Create-receipt capacities, encoded
registry bytes, command queue capacity, retained cuts, retained page tokens,
Watch capacity, maximum page rows and response bytes. It also contains the
existing placement policy and bundle limits. No resource budget is inferred
from machine capacity. Namespace ownership is exclusive: a competing conditional
write fails rather than overwriting another manager's registry.

## Client Operations

| Method | Request and Result |
| --- | --- |
| `Get` | Name and optional issued `at`; complete entry and `resolved_at`. |
| `List` | Optional issued `at` and bounded `page`; ordered entries, `resolved_at` and opaque `next_page_token`. |
| `Watch` | Optional name and issued `at`; opening snapshot followed by ordered changes or explicit resynchronization. Empty name selects all entries. |
| `Create` | Request identifier, opaque name, placement and complete writable value; original entry and durable receipt. |
| `Set` | Name, whole `PluginValue { enabled }` and expected revision from Get; entry and durable receipt. |
| `Delete` | Name and expected revision; durable removal receipt after activation cleanup. |
| `Restart` | Name and expected revision; replaces an enabled entry's activation. |

Messages follow `Plugins<Method>Request` and `Plugins<Method>Response`.
Placement is immutable. Use Delete/Create to change it. Set carries all writable
fields and refuses stale revisions with the action to get again and retry.
The entry also reports its runtime state, last connected description, catalog
digest, activation identity, current managed process identity and failure.
Unavailable entries stay in the set and remain manageable.

Create replay receipts are retained for the namespace's lifetime. Resending the
same identifier and normalized payload returns the original response, including
after host restart or entry deletion, without starting another process. A changed
payload conflicts. Exhausting the configured receipt capacity refuses new
creates; keys never silently expire and repeat work.

Latest reads resolve the manager's latest durable snapshot without causing a
flush. Return `resolved_at` unchanged for consistency. A page token binds its
cut, ordering, page rows and byte budget. Cuts and tokens have configured bounded
retention and expire across manager restart; foreign, unissued or expired
continuations explicitly fail. Watch subscribes atomically with its opening
snapshot and replays retained changes from an issued cut. A slow observer whose
event history expires receives `resync_required` and must open a new snapshot.
A disconnected Watch never deletes its entries.

## Bundles and Processes

Upload the existing bundle format through the library's file-production session:
Create the production, Append bounded ordered chunks, Get progress to resume,
and Publish the verified content. Supply that immutable published file reference
and the archive SHA-256 in `PluginManaged`. `PublishedContent` reads the retained
published file through storage; a narrow `ContentSource` trait supports another
published-content owner without adding an upload protocol.

Create checks the archive digest, manifest and per-file digests using the existing
runtime bundle reader. It requires one nonempty native sidecar executable for the
configured target before extraction or startup. Installed files live beneath the
manager's own directory. The executable digest is persisted and checked again
before each activation. Invalid content never creates an entry or starts a process.

Managed executables receive `HARMOS_ADDRESS=127.0.0.1:0` and announce their actual
bound endpoint through the existing readiness line. Windows launches use
`CREATE_NO_WINDOW`. Existing attachment code owns catalog admission, dispatch,
streams, reconnect and process supervision. A managed process exit reports an
unavailable entry with its failure; explicit Restart replaces it. Remote
attachments reconnect under their existing placement policy. Delete stops and
reaps managed processes and removes their installation. A remote process remains
owned by its external launcher.

On host restart, persisted placement and enabled intent are restored. Enabled
managed entries start from their installation, remote entries reconnect, and
disabled entries remain stopped with no process. Startup and connection failures
are reported per entry. Shut down through `runtime.shutdown().await` so the
manager and attachments settle before storage closes.

## Product Responsibilities

The product supplies the existing external authorizer and grant policy.
Management operations are Public declarations whose request grant is scoped to
`plugins`; public visibility alone does not grant a caller a product role.
The native host remains trusted. Bind opaque work scope on the existing
`runtime.operations.with_scope(scope)` handle when invoking hosted operations;
the manager never interprets that scope or treats it as authorization.

A product adopting this manager removes its own entry registry, placement
persistence, bundle installer, enable/restart/remove handlers and duplicated
connection, dispatch, stream and process-launch implementation. Its client
interface calls the generated `Plugins` service. Catalog interpretation, selection
of plugins for a unit of work, per-plugin settings and secrets stay with the
product. Whether settings and secrets merit a shared library contract remains an
open design question; this manager adds no settings or credential schema.
