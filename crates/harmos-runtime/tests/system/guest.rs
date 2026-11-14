/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

use harmos_protocol::{
    client::{
        PluginState as EntryState, PluginValue, PluginsDeleteRequest, PluginsGetRequest, PluginsListRequest, PluginsReplaceRequest,
        PluginsSetRequest, PluginsWatchRequest, plugin_change,
        plugin_placement::Kind,
        plugins::{SettingsListRequest, SettingsSetRequest},
        plugins_watch_response::Event,
    },
    v1::{SettingValue, setting_value::Value},
};
use harmos_runtime::plugin::manager::Management;
use harmos_runtime::{Identifier, Runtime, guest::PluginState};
use prost::Message as _;
use schematic_operations::protocol::{Connection, Point, ProposeRoute, Route, Snapshot, ValidateSchematic, Validation};
use std::{num::NonZeroUsize, sync::Arc, time::SystemTime};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */

#[tokio::test]
async fn guest_composition_walks() {
    let _report = crate::report::system();
    // Boot Without a Guest Operation
    let (clock, _) = harmos_runtime::clock::Clock::virtual_at(SystemTime::UNIX_EPOCH);
    let (storage, entries, root) = super::guest_contract::support::entries();
    let runtime = Runtime::<crate::support::Bank>::launch_with(crate::support::Ledger::default(), (), |setup| {
        setup.resource(root);
        setup.settings().clock(clock).storage(&storage).plugins(entries.clone());
        setup.settings().register(crate::support::catalog());
        setup.settings().operation_settings(harmos_runtime::operations::Settings {
            preparation_bytes: NonZeroUsize::new(4096),
            ..Default::default()
        });
    })
    .await
    .unwrap();
    let runtime = Arc::new(runtime);
    entries.start(&runtime).await.unwrap();
    let route = ProposeRoute {
        from: Some(Point { x: 0, y: 0 }),
        to: Some(Point { x: 3, y: 4 }),
    };
    assert!(runtime.operations.call_encoded("schematic.route", route.encode_to_vec()).await.is_err());

    // Load the Same Unary Builder Values Used by the Native SDK
    let bytes = crate::support::fixtures::component("schematic-guest");
    let host = harmos_runtime::guest::Host::new(&runtime).unwrap();
    let loaded = host.load(&bytes).unwrap();
    assert_eq!(loaded.declaration().id, "schematic_tools");
    let declared: Vec<_> = loaded.declaration().operations.iter().map(|operation| operation.id.as_str()).collect();
    assert_eq!(declared, ["schematic.validate", "schematic.route"]);
    runtime
        .plugin
        .register_guest(
            loaded,
            harmos_runtime::config::PluginSettings {
                host_operations: [Identifier::new("harmos.plugin.host.v1.Settings/List").unwrap()].into_iter().collect(),
                configuration: b"null".to_vec(),
                ..Default::default()
            },
            harmos_runtime::operations::Lane::default(),
        )
        .await
        .unwrap();
    runtime.plugin.initialize().await.unwrap();
    let original = runtime.plugin.status("schematic_tools").unwrap();
    assert_eq!(original.state, PluginState::Ready);

    // Route and Validate With the Authoritative Application Messages
    let bytes = runtime.operations.call_encoded("schematic.route", route.encode_to_vec()).await.unwrap();
    let answer = Route::decode(bytes.as_slice()).unwrap();
    assert_eq!(answer.points, [Point { x: 0, y: 0 }, Point { x: 3, y: 0 }, Point { x: 3, y: 4 }]);
    let request = ValidateSchematic {
        schematic: Some(Snapshot {
            symbols: vec![1, 1],
            connections: vec![Connection { from: 1, to: 2 }],
        }),
    };
    let bytes = runtime.operations.call_encoded("schematic.validate", request.encode_to_vec()).await.unwrap();
    assert_eq!(Validation::decode(bytes.as_slice()).unwrap().diagnostics.len(), 2);

    // Read and Change the Guest's Stored Typed Settings Through Its Entry
    let name = "schematic_tools".to_owned();
    let listed = entries.list(PluginsListRequest { at: None }).await.unwrap();
    assert_eq!(listed.entries.len(), 1);
    assert!(matches!(listed.entries[0].placement.as_ref().unwrap().kind, Some(Kind::Guest(_))));
    let before = entries.settings_list(SettingsListRequest { name: name.clone(), at: None }).await.unwrap();
    let old_cut = before.resolved_at.clone();
    let mut value = before.value.unwrap();
    value.values.insert("horizontal_first".into(), SettingValue { value: Some(Value::Flag(false)) });
    entries
        .settings_set(SettingsSetRequest {
            name: name.clone(),
            value: Some(value.clone()),
            expected_revision: before.revision.clone(),
            request_id: "guest-settings".into(),
        })
        .await
        .unwrap();
    assert!(
        entries
            .settings_set(SettingsSetRequest {
                name: name.clone(),
                value: Some(value),
                expected_revision: before.revision,
                request_id: "guest-stale-settings".into(),
            })
            .await
            .is_err()
    );
    let answer = runtime.operations.call_encoded("schematic.route", route.encode_to_vec()).await.unwrap();
    assert_eq!(Route::decode(answer.as_slice()).unwrap().points, [Point { x: 0, y: 0 }, Point { x: 0, y: 4 }, Point { x: 3, y: 4 }]);

    // Replace the Activation and Keep Answering the Same Contract
    runtime.plugin.reload("schematic_tools").await.unwrap();
    let reloaded = runtime.plugin.status("schematic_tools").unwrap();
    assert_eq!(reloaded.state, PluginState::Ready);
    assert_ne!(reloaded.generation, original.generation);
    assert!(runtime.operations.call_encoded("schematic.route", route.encode_to_vec()).await.is_ok());
    let answer = runtime.operations.call_encoded("schematic.route", route.encode_to_vec()).await.unwrap();
    assert_eq!(Route::decode(answer.as_slice()).unwrap().points[1], Point { x: 0, y: 4 });
    let entry = entries.get(PluginsGetRequest { name: name.clone(), at: None }).await.unwrap().entry.unwrap();
    // Disable the Entry, Preserve Its Module, and Reenable It Through Management
    let disabled = entries
        .set(PluginsSetRequest {
            name: name.clone(),
            expected_revision: entry.revision,
            value: Some(PluginValue { enabled: false }),
        })
        .await
        .unwrap()
        .entry
        .unwrap();
    runtime.plugin.initialize().await.unwrap();
    assert!(runtime.operations.call_encoded("schematic.route", route.encode_to_vec()).await.is_err());
    let mut changes = entries.watch(PluginsWatchRequest { name: name.clone(), at: None }).await.unwrap();
    entries
        .set(PluginsSetRequest {
            name: name.clone(),
            expected_revision: disabled.revision,
            value: Some(PluginValue { enabled: true }),
        })
        .await
        .unwrap();
    loop {
        let event = changes.recv().await.unwrap().event.unwrap();
        if let Event::Change(change) = event
            && let Some(plugin_change::Kind::Entry(entry)) = change.kind
            && entry.value.is_some_and(|value| value.enabled)
            && entry.state == EntryState::Ready as i32
        {
            break;
        }
    }
    let answer = runtime.operations.call_encoded("schematic.route", route.encode_to_vec()).await.unwrap();
    assert_eq!(Route::decode(answer.as_slice()).unwrap().points[1], Point { x: 0, y: 4 });
    let entry = entries.get(PluginsGetRequest { name: name.clone(), at: None }).await.unwrap().entry.unwrap();
    let refusal = entries
        .replace(PluginsReplaceRequest {
            name: name.clone(),
            expected_revision: entry.revision.clone(),
            request_id: "guest-replace".into(),
            placement: entry.placement.clone(),
        })
        .await
        .unwrap_err();
    assert_eq!(refusal.0.code, "managed_required");
    entries
        .delete(PluginsDeleteRequest {
            name: name.clone(),
            expected_revision: entry.revision,
        })
        .await
        .unwrap();
    assert!(entries.list(PluginsListRequest { at: None }).await.unwrap().entries.is_empty());
    assert!(entries.settings_list(SettingsListRequest { name, at: old_cut }).await.is_err());
    assert!(runtime.plugin.status("schematic_tools").is_none());
    assert!(runtime.operations.call_encoded("schematic.route", route.encode_to_vec()).await.is_err());
    runtime.shutdown().await.unwrap();
}
