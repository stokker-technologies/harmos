# Plugin Management

`harmos_runtime::plugin::manager` owns a persistent set of hosted plugins.
Its client-facing `harmos.client.v1.Plugins` service is separate from
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
Rust clients use `harmos_protocol::client::plugins_client::PluginsClient`;
`harmos::client::Client` exposes the descriptor-generated `client.plugins`
hierarchy for the same request/response contracts.
Native hosts can also use the `Management` trait on `Manager` directly.

`PluginManagerSettings` supplies entry and mutation-receipt capacities, encoded
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
| `Replace` | Name, managed bundle placement, expected revision and request identity; replaces the bundle in place and returns the entry and receipt. |
| `Settings.List` | Name and optional issued cut; typed declarations, complete values, settings revision and resolved cut. |
| `Settings.Set` | Name, complete typed value, expected settings revision and request identity; applied value, revision and durable receipt. |

Messages follow `Plugins<Method>Request` and `Plugins<Method>Response`.
Replace changes a managed bundle while retaining the entry name, settings and product attachments. It validates the replacement description, contracts and sample channels before activating it. Failed staging leaves the previous bundle serving. Remote entries cannot be replaced by this operation. Set carries all writable
fields and refuses stale revisions with the action to get again and retry.
The entry also reports its runtime state, last connected description, catalog
digest, activation identity, current managed process identity and failure.
Unavailable entries stay in the set and remain manageable.

Create replay receipts are retained for the namespace's lifetime, including after
entry deletion. Settings.Set and Replace receipts live beside their entry and are
purged when it is deleted. Resending an identifier and normalized payload replays
the retained original response, including after restart, without starting another
process; changed input conflicts. `PluginManagerSettings.create_receipts` bounds
the combined retained mutation receipts. Exhausting it refuses new mutations;
retained keys never silently expire and repeat work.

Latest reads resolve the manager's latest durable snapshot without causing a
flush. List returns every configured entry in one response. Return `resolved_at`
unchanged for consistency. Cuts have configured bounded
retention and expire across manager restart; foreign, unissued or expired
continuations explicitly fail. Watch subscribes atomically with its opening
snapshot and replays retained changes from an issued cut. A slow observer whose
event history expires receives `resync_required` and must open a new snapshot.
A disconnected Watch never deletes its entries.

## Bundles and Processes

Upload the existing bundle format through the product's own upload operation over the library's private native file-production operations:
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
of plugins for a unit of work and secrets stay with the product. Product-defined
catalog sections describe product resources, including secret requirements.
The library neither stores nor interprets those sections.

## Typed Settings

A plugin reports `harmos.v1.SettingsDeclarations` in `Catalog.sections`. Each
setting has a name, title, description and typed default. `SettingValue` carries
one flag, integer, finite number, text or duration. List materializes defaults;
Set replaces the complete map, rejects unknown or missing names and checks each
value's variant against its declared default. Settings have a revision separate
from the plugin entry revision. Resending the same request identity and normalized
value replays its original receipt, including after restart; changed input refuses.

`harmos.plugin.host.v1.Settings/List` reads only the calling entry's values.
The SDK handler remains named `context`. Running plugins explicitly read settings;
there is no change notification. Host stream intervals configured by setting name
use the value selected for that activation; restart to apply a changed interval.

Product plugin-host handlers declare the public resource
`harmos_runtime::plugin::grpc::CallingPlugin`. Its `name` is the management entry
identity when the attachment belongs to an entry,
and `None` for a direct attachment. Its `description` is the admitted instance
description. An unmanaged attachment cannot select a stored entry through its
reported ID and cannot read library-stored settings. Direct attachments receive
opaque provider identities; protected self-calls bind to their physical activation.
The host
inserts this resource when routing a plugin child; request fields do not select
its identity. Products can inspect their own reported sections before serving data.

Delete removes library settings and their retained read images with the entry in
one durable registry commit and
publishes `PluginChange.deleted` through Watch. A product removes its own per-plugin
data from that in-process watch and reconciles against List at startup. Product
cleanup is eventual; it is not part of the library's deletion transaction.
