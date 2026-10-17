/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */
use schematic_domain as schematic;

/* === Re-Exports === */

/* === Imports === */
// Standard Library Imports
use std::num::NonZeroUsize;
// Third Party Imports

// Local Imports
use crate::support;
use harmos_runtime::Runtime;
use harmos_runtime::plugin::PluginState;
use schematic::model::{Connection, Point, Snapshot};
use schematic::operations::Operations;
use schematic::operations::schematic::{analyze, propose, validate};

/* === Constants === */
const PLUGIN: &str = "schematic-tools";

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */
#[harmos_runtime::lanes]
enum Lanes {
    #[default]
    Open,
}

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

/* === Private Functions === */
fn route() -> propose::ProposeRoute {
    propose::ProposeRoute {
        from: Point { x: 0, y: 0 },
        to: Point { x: 3, y: 4 },
    }
}

fn corner() -> [Point; 3] {
    [
        Point { x: 0, y: 0 },
        Point { x: 3, y: 0 },
        Point { x: 3, y: 4 },
    ]
}

#[tokio::test]
async fn guest_composition_walks() {
    // Scene 1: The Host Boots Without Any Guest Operation
    let runtime = Runtime::builder(support::Ledger::default(), ())
        .register(Lanes::catalog())
        .register(support::catalog())
        .operation_settings(harmos_runtime::operations::Settings {
            preparation_bytes: Some(NonZeroUsize::new(4096).unwrap()),
            ..Default::default()
        })
        .runtime()
        .await
        .expect("an empty ledger has nothing to recover");
    assert!(runtime.plugin.status(PLUGIN).is_none());

    // Scene 2: The Stamped Schematic Guest Declares Itself Before Instantiation
    let bytes = support::fixtures::component("schematic-guest");
    let host = harmos_runtime::guest::Host::new(&runtime).unwrap();
    let loaded = host.load(&bytes).unwrap();
    let declaration = loaded.declaration();
    assert_eq!(declaration.id, PLUGIN);
    let mut declared: Vec<_> = declaration
        .operations
        .iter()
        .map(|operation| operation.id.to_string())
        .collect();
    declared.sort();
    assert_eq!(
        declared,
        ["schematic.analyze", "schematic.route", "schematic.validate"]
    );

    // Scene 3: Registration And Initialization Make The Guest Ready
    let settings = harmos_runtime::config::PluginSettings {
        configuration: b"null".to_vec(),
        ..Default::default()
    };
    runtime
        .plugin
        .register_guest::<Operations>(loaded, settings, Lanes::Open)
        .await
        .unwrap();
    runtime.plugin.initialize().await.unwrap();
    let ready = runtime.plugin.status(PLUGIN).unwrap();
    assert_eq!(ready.state, PluginState::Ready);

    // Scene 4: A Typed Caller Routes Through The Guest
    let routed = runtime
        .operations
        .call(route())
        .await
        .unwrap();
    assert_eq!(routed.points, corner());

    // Scene 5: Validation Reports Every Diagnostic Of A Broken Schematic
    let validated = runtime
        .operations
        .call(validate::ValidateSchematic {
            schematic: Snapshot {
                symbols: vec![1, 1],
                connections: vec![Connection { from: 1, to: 2 }],
            },
        })
        .await
        .unwrap();
    assert_eq!(
        validated.diagnostics,
        [
            "Duplicate Symbol IDs",
            "Connection 1 To 2 References A Missing Symbol"
        ]
    );

    // Scene 6: Analysis Streams Its Findings Before Its Response
    let settings = harmos_runtime::operations::StreamSettings {
        items: NonZeroUsize::MIN,
        bytes: NonZeroUsize::new(4096).unwrap(),
    };
    let request = analyze::AnalyzeSchematic {
        schematic: Snapshot {
            symbols: vec![1, 2],
            connections: vec![Connection { from: 1, to: 2 }],
        },
    };
    let mut download = runtime
        .operations
        .download::<analyze::AnalyzeSchematic>(request, settings)
        .unwrap();
    for expected in ["2 symbols", "1 connections", "Analysis complete"] {
        assert_eq!(download.next().await.unwrap().unwrap().text, expected);
    }
    assert!(download.next().await.unwrap().is_none());
    assert_eq!(download.join().await.unwrap().checked, 3);

    // Scene 7: Reloading Replaces The Generation And Keeps Answering
    runtime.plugin.reload(PLUGIN).await.unwrap();
    let reloaded = runtime.plugin.status(PLUGIN).unwrap();
    assert_eq!(reloaded.state, PluginState::Ready);
    assert_ne!(reloaded.generation, ready.generation);
    let routed = runtime
        .operations
        .call(route())
        .await
        .unwrap();
    assert_eq!(routed.points, corner());

    // Scene 8: Stopping The Guest Withdraws Its Operations
    runtime.plugin.stop(PLUGIN).await.unwrap();
    assert!(
        runtime
            .operations
            .call(route())
            .await
            .is_err()
    );

    // Scene 9: The Host Stops Cleanly
    runtime.stop().await.unwrap();
}

/* === Tests === */
