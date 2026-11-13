# Plugin Management

`harmos_runtime::plugin::manager` owns a persistent set of hosted plugins.
Its client-facing `harmos.v1.Plugins` service is separate from
`harmos.plugin.v1.Instance`, which each plugin executable implements.

## Host Composition

Register the manager's object catalog with the storage backend and storage
manager. Supply an exclusive manager namespace, installation directory, native
bundle target and every capacity in `PluginManagerSettings`. Construct the
manager with the storage handle, register it with
`setup.settings().plugins(manager.clone())`, launch the runtime in an `Arc`, then
call `manager.start(&runtime).await`. Storage must already be started; startup
fails explicitly if its conditional object primitive is unavailable.

```rust
use std::sync::Arc;
use harmos_runtime::plugin::manager::Manager;

// Include plugin::manager::catalog() in the storage and backend object catalogs.
let manager = Manager::new(storage.clone(), settings)?;
// In the runtime's existing setup closure:
setup.settings().storage(&storage).plugins(manager.clone());
// After the configured Runtime has launched:
let runtime = Arc::new(runtime);
manager.start(&runtime).await?;
```

The host's existing gRPC listener projects the registered operations. Generated
Rust clients use `harmos_protocol::v1::plugins_client::PluginsClient`;
`harmos::client::Client` exposes the descriptor-generated `client.plugins`
hierarchy for the same request/response contracts.
Native hosts can also use the `Management` trait on `Manager` directly.

`PluginManagerSettings` supplies entry and Create-receipt capacities, encoded
registry bytes, command queue capacity, retained cuts,
Watch capacity and Get/Watch response bytes. It also contains the
existing placement policy and bundle limits. No resource budget is inferred
from machine capacity. Namespace ownership is exclusive: a competing conditional
write fails rather than overwriting another manager's registry.

## Client Operations

| Method | Request and Result |
| --- | --- |
| `Get` | Name and optional issued `at`; complete entry and `resolved_at`. |
| `List` | Optional issued `at`; the complete configured entry set in identifier order and `resolved_at`. The carrier message ceiling is the sole List response guard. |
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
flush. List returns every configured entry in one response. Return `resolved_at`
unchanged for consistency. Cuts have configured bounded
retention and expire across manager restart; foreign, unissued or expired
continuations explicitly fail. Watch subscribes atomically with its opening
snapshot and replays retained changes from an issued cut. A slow observer whose
event history expires receives `resync_required` and must open a new snapshot.
A disconnected Watch never deletes its entries.

## Bundles and Processes

Upload the existing bundle format through the library's file-production session:
Create the production, Append bounded ordered chunks, Get progress to resume,
and Publish the verified content. Supply that immutable published file reference
and the archive SHA-256 in `PluginManaged`. The manager opens one retained
`storage.open_content(&reference, None, None)` read. Application-owned bundles use
StorageManager's registered `ContentSource`, shared with ranged reads; no bundle-only
content adapter or constructor argument remains. The application decides whether
an unscoped host installation may resolve its reference. This supports another
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
product. The owner's boundary decision closes the settings/secrets question:
generic catalog declarations and context host-value delivery belong to their
existing library owners; stores, schemas and credential policy remain product-owned.
