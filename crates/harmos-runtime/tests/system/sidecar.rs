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
// Standard Library Imports
use std::num::NonZeroUsize;

// Third Party Imports
use futures_util::StreamExt;

// Local Imports
use crate::support;
use harmos_runtime::Runtime;
use harmos_runtime::config::{PluginSettings, PluginSidecarSettings};
use harmos_runtime::operations::{FailureKind, StreamSettings};
use harmos_runtime::plugin::PluginState;
use harmos_runtime::sidecar::Executable;
use harmos_runtime::streams::Item;
use schematic_domain::model::{Connection, Point, Snapshot};
use schematic_domain::model::schematic::progress::AnalysisProgress;
use schematic_domain::operations::{AnalyzeExchange, AnalyzeUpload, ListDirectories, Native, ProposeRoute};

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
fn snapshot() -> Snapshot {
    Snapshot {
        symbols: vec![1, 2],
        connections: vec![Connection { from: 1, to: 2 }],
    }
}

fn stream_settings() -> StreamSettings {
    StreamSettings {
        items: NonZeroUsize::MIN,
        bytes: NonZeroUsize::new(4096).unwrap(),
    }
}

fn plugin_settings() -> PluginSidecarSettings {
    PluginSidecarSettings {
        plugin: PluginSettings {
            configuration: b"null".to_vec(),
            host_operations: vec![harmos_runtime::Identifier::new("host.publish").unwrap()],
            ..Default::default()
        },
        ..Default::default()
    }
}


/* === Tests === */

#[tokio::test]
async fn sidecar_composition_walks() {
    // Scene 1: The Host Owns An Explicit Working Directory And Stamped Child
    let directory = tempfile::tempdir().unwrap();
    std::fs::create_dir(directory.path().join("boards")).unwrap();
    std::fs::write(directory.path().join("notes.txt"), b"not a directory").unwrap();
    let executable = Executable::at(support::fixtures::artifact("schematic-sidecar"))
        .unwrap()
        .directory(directory.path());
    assert_eq!(executable.declaration().id, PLUGIN);
    let runtime = Runtime::builder(support::Ledger::default(), ())
        .register(Lanes::catalog())
        .register(support::catalog())
        .runtime()
        .await
        .unwrap();
    runtime.plugin.register_sidecar::<Native>(executable.clone(), plugin_settings(), Lanes::Open).await.unwrap();
    runtime.plugin.initialize().await.unwrap();
    let ready = runtime.plugin.status(PLUGIN).unwrap();
    assert_eq!(ready.state, PluginState::Ready);
    let directories = runtime.operations.call(ListDirectories {}).await.unwrap();
    assert_eq!(directories.directories, ["boards"]);

    // Scene 2: A Unary Operation Crosses The Same Native Transport
    let routed = runtime.operations.call(ProposeRoute {
        from: Point { x: 0, y: 0 }, to: Point { x: 3, y: 4 },
    }).await.unwrap();
    assert_eq!(routed.points, [Point { x: 0, y: 0 }, Point { x: 3, y: 0 }, Point { x: 3, y: 4 }]);

    // Scene 3: Upload Progress Proves The Child Holds Its Shared Analysis Permit
    let mut progress = runtime.streams.watch::<AnalysisProgress>();
    let mut upload = runtime.operations.upload(AnalyzeUpload { title: "Held Upload".into() }, stream_settings()).unwrap();
    upload.send(snapshot()).await.unwrap();
    let Item::Row(row) = progress.next().await.unwrap() else { panic!("the progress window lost a row") };
    assert_eq!((row.value.title.as_str(), row.value.checked), ("Held Upload", 1));

    // Scene 4: A Bidirectional Call Waits For That Permit Until Cancellation Settles
    let mut exchange = runtime.operations.exchange(AnalyzeExchange { title: "Waiting Exchange".into() }, stream_settings(), stream_settings()).unwrap();
    exchange.send(snapshot()).await.unwrap();
    {
        let next = exchange.next();
        futures_util::pin_mut!(next);
        assert!(futures_util::poll!(next).is_pending());
    }
    upload.cancel_and_settle().await.unwrap();
    assert_eq!(exchange.next().await.unwrap().unwrap().text, "2 Symbols, 1 Connections");
    exchange.finish_input();
    assert!(exchange.next().await.unwrap().is_none());
    assert_eq!(exchange.join().await.unwrap().checked, 1);
    let Item::Row(row) = progress.next().await.unwrap() else { panic!("the progress window lost a row") };
    assert_eq!((row.value.title.as_str(), row.value.checked), ("Waiting Exchange", 1));

    // Scene 5: Upload EOF Returns Its Final Count And Invalid Input Stays Typed
    let mut completed = runtime.operations.upload(AnalyzeUpload { title: "Completed Upload".into() }, stream_settings()).unwrap();
    completed.send(snapshot()).await.unwrap();
    completed.finish_input();
    assert_eq!(completed.join().await.unwrap().checked, 1);
    let mut invalid = runtime.operations.upload(AnalyzeUpload { title: String::new() }, stream_settings()).unwrap();
    invalid.finish_input();
    let error = invalid.join().await.unwrap_err();
    assert_eq!(error.failure().unwrap().code(), FailureKind::Invalid);

    // Scene 6: Explicit Reload Replaces The Generation And Serves The Same Directory
    drop(progress);
    runtime.plugin.reload_sidecar::<Native>(executable, plugin_settings(), Lanes::Open).await.unwrap();
    let reloaded = runtime.plugin.status(PLUGIN).unwrap();
    assert_eq!(reloaded.state, PluginState::Ready);
    assert_ne!(ready.generation, reloaded.generation);
    assert_eq!(runtime.operations.call(ListDirectories {}).await.unwrap().directories, ["boards"]);

    // Scene 7: Shutdown Reaps The Owned Child And Withdraws Its Operations
    runtime.plugin.stop(PLUGIN).await.unwrap();
    assert!(runtime.operations.call(ListDirectories {}).await.is_err());
    runtime.stop().await.unwrap();
}
