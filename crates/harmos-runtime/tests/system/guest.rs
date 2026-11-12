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

use harmos_runtime::{Runtime, guest::PluginState};
use prost::Message as _;
use schematic_operations::protocol::{Connection, Point, ProposeRoute, Route, Snapshot, ValidateSchematic, Validation};
use std::{num::NonZeroUsize, time::SystemTime};

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
    let runtime = Runtime::<crate::support::Bank>::launch_with(crate::support::Ledger::default(), (), |setup| {
        setup.settings().clock(clock);
        setup.settings().register(crate::support::catalog());
        setup.settings().operation_settings(harmos_runtime::operations::Settings {
            preparation_bytes: NonZeroUsize::new(4096),
            ..Default::default()
        });
    })
    .await
    .unwrap();
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

    // Replace the Activation and Keep Answering the Same Contract
    runtime.plugin.reload("schematic_tools").await.unwrap();
    let reloaded = runtime.plugin.status("schematic_tools").unwrap();
    assert_eq!(reloaded.state, PluginState::Ready);
    assert_ne!(reloaded.generation, original.generation);
    assert!(runtime.operations.call_encoded("schematic.route", route.encode_to_vec()).await.is_ok());
    runtime.plugin.stop("schematic_tools").await.unwrap();
    assert!(runtime.operations.call_encoded("schematic.route", route.encode_to_vec()).await.is_err());
    runtime.shutdown().await.unwrap();
}
