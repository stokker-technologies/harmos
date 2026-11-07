## Design

Guest and Sidecar system facts retain their schema in the existing journal payload and snapshot tail through Prepared::system. Work facts retain their existing host representation. Recovery validates the stored schema/revision pair and reconstructs plugin facts without consulting the current plugin. History exposes the schema and original payload; it does not project historical plugin payloads into the host state or current plugin definition.

Generic storage records, objects and streams already persist StorageSchema graphs and reference exact SchemaKeys. Queries resolve those stored definitions. Typed object reads require the requested definition and explicitly refuse a different shape; generic historical queries remain available. Unknown object definitions receive retain-all policy, preventing absent or changed catalogs from failing background pruning or deleting their history. Current definitions continue governing new writes; no automatic state conversion occurs.

PluginScope contributions comprise operations, services/observers, streams, listeners and inert registrations/sections. Only streams have plugin-registered persistent schemas. Listener cursors and activation generations are memory-only. Transactions and typed state record catalogs belong to the compiled Application; object catalogs belong to host storage composition. A plugin cannot register those through its scope. Host shape refusal is unchanged; no ownership inference from arbitrary type names is introduced.

## Inventory and Acceptance

One writer owns all changes in the main checkout. Edit domain state/envelope.rs and record.rs, storage catalog/retention handling and JSON pruning, existing runtime shape_versions.rs tests, README.md and docs/product-guide.md. Reuse existing storage history/query APIs. Validate one-root plugin record/object evolution and absence, journal and snapshot fact history, and existing named host refusal. Run affected crate tests, the CI test-target build and lint:rust through mise with CARGO_BUILD_JOBS=8. The user explicitly excludes full lanes and simulation.
