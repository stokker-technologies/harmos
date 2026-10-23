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
use schematic_domain::model::schematic::progress::AnalysisProgress;
use schematic_domain::model::{Connection, Point, Snapshot};
use schematic_domain::operations::{AnalyzeExchange, AnalyzeUpload, ListDirectories, Native, ProposeRoute};

/* === Constants === */

const PLUGIN: &str = "schematic-tools";
const ENV_SCENE: &str = "HARMOS_SIDECAR_ENV_SCENE";

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

// The Host Declares No Stream: The Sidecar Owns Its Progress Stream And The Host Only Watches It.
struct Host;

/* === Enums === */

#[harmos_runtime::lanes]
enum Lanes {
    #[default]
    Open,
}

/* === Traits === */

/* === Implementations === */

impl harmos_runtime::Application for Host {
    type State = ();
    type Transaction = harmos_runtime::state::Never;
    type Record = ();
    type Streams = ();
    type Resources = ();
}

/* === Public Functions === */

/* === Private Functions === */

fn snapshot() -> Snapshot {
    Snapshot { symbols: vec![1, 2], connections: vec![Connection { from: 1, to: 2 }] }
}

fn stream_settings() -> StreamSettings {
    StreamSettings { items: NonZeroUsize::MIN, bytes: NonZeroUsize::new(4096).unwrap() }
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
    if std::env::var(ENV_SCENE).as_deref() != Ok("1") {
        // The Current-Thread Test Host Tolerates This Ambient Setting; A Native
        // Multithread Sidecar Must Clear It Before A Fresh Valid Generation Starts.
        let mut command = tokio::process::Command::new(std::env::current_exe().unwrap());
        #[cfg(windows)]
        command.creation_flags(0x08000000);
        let output = command
            .args(["--exact", "sidecar::sidecar_composition_walks", "--nocapture"])
            .env(ENV_SCENE, "1")
            .env("TOKIO_WORKER_THREADS", "0")
            .kill_on_drop(true)
            .output()
            .await
            .unwrap();
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(output.status.success(), "owned environment scene failed:\n{stdout}\n{stderr}");
        assert!(stdout.contains("test result: ok. 1 passed; 0 failed;"), "the exact child story must run once:\n{stdout}\n{stderr}");
        return;
    }
    assert_eq!(std::env::var("TOKIO_WORKER_THREADS").unwrap(), "0");

    // Scene 1: The Host Owns An Explicit Working Directory And Stamped Child
    let directory = tempfile::tempdir().unwrap();
    std::fs::create_dir(directory.path().join("boards")).unwrap();
    std::fs::write(directory.path().join("notes.txt"), b"not a directory").unwrap();
    let executable = Executable::at(support::fixtures::artifact("schematic-sidecar")).unwrap().directory(directory.path());
    assert_eq!(executable.declaration().id, PLUGIN);
    // Typed Native Calls Prepare Their Encoded Requests Within An Explicit Host Budget
    let runtime = Runtime::<Host>::builder((), ())
        .register(Lanes::catalog())
        .operation_settings(harmos_runtime::operations::Settings {
            preparation_bytes: Some(NonZeroUsize::new(64 * 1024).unwrap()),
            ..Default::default()
        })
        .runtime()
        .await
        .unwrap();
    // Scene 2: Locked Tokio Rejects Zero Workers Before The Native Plugin Is Ready.
    // This Exercises A Real Startup Crash Through An Existing Environment Option.
    let crashing = executable.clone().variable("TOKIO_WORKER_THREADS", "0");
    runtime.plugin.register_sidecar::<Native>(crashing, plugin_settings(), Lanes::Open).await.unwrap();
    assert!(runtime.plugin.initialize().await.is_err());
    let failed = runtime.plugin.status(PLUGIN).unwrap();
    assert_eq!(failed.state, PluginState::Failed);
    assert_eq!(failed.attempts, 1);
    assert!(failed.last_failure.is_some());
    assert!(failed.next_retry.is_none());
    assert!(runtime.operations.call(ListDirectories {}).await.is_err());
    runtime.plugin.stop(PLUGIN).await.unwrap();
    runtime.plugin.stop(PLUGIN).await.unwrap();
    let settled = runtime.plugin.status(PLUGIN).unwrap();
    assert!(!settled.cleanup_pending);
    assert_eq!(settled.attempts, failed.attempts);

    // A Fresh Valid Executable Recovers Through The Same Public Plugin Owner.
    runtime.plugin.reload_sidecar::<Native>(executable.clone(), plugin_settings(), Lanes::Open).await.unwrap();
    let ready = runtime.plugin.status(PLUGIN).unwrap();
    assert_ne!(ready.generation, failed.generation);
    assert_eq!(ready.state, PluginState::Ready);
    let directories = runtime.operations.call(ListDirectories {}).await.unwrap();
    assert_eq!(directories.directories, ["boards"]);

    // Scene 3: A Unary Operation Crosses The Same Native Transport
    let routed = runtime.operations.call(ProposeRoute { from: Point { x: 0, y: 0 }, to: Point { x: 3, y: 4 } }).await.unwrap();
    assert_eq!(routed.points, [Point { x: 0, y: 0 }, Point { x: 3, y: 0 }, Point { x: 3, y: 4 }]);

    // Scene 4: Upload Progress Proves The Child Holds Its Shared Analysis Permit
    let published = runtime.streams.publication("schematic.analysis_progress", 1).expect("the ready sidecar owns its progress stream");
    assert_eq!(published.definition().owner(), Some(PLUGIN));
    let mut progress = published.watch();
    let upload = runtime.operations.upload(AnalyzeUpload { title: "Held Upload".into() }, stream_settings()).unwrap();
    upload.send(snapshot()).await.unwrap();
    let Item::Row(row) = progress.next().await.unwrap() else { panic!("the progress window lost a row") };
    let row = <AnalysisProgress as harmos_runtime::Message>::decode(&row.value.payload).unwrap();
    assert_eq!((row.title.as_str(), row.checked), ("Held Upload", 1));

    // Scene 5: A Bidirectional Call Waits For That Permit Until Cancellation Settles
    let mut exchange =
        runtime.operations.exchange(AnalyzeExchange { title: "Waiting Exchange".into() }, stream_settings(), stream_settings()).unwrap();
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
    let row = <AnalysisProgress as harmos_runtime::Message>::decode(&row.value.payload).unwrap();
    assert_eq!((row.title.as_str(), row.checked), ("Waiting Exchange", 1));

    // Scene 6: Upload EOF Returns Its Final Count And Invalid Input Stays Typed
    let mut completed = runtime.operations.upload(AnalyzeUpload { title: "Completed Upload".into() }, stream_settings()).unwrap();
    completed.send(snapshot()).await.unwrap();
    completed.finish_input();
    assert_eq!(completed.join().await.unwrap().checked, 1);
    let mut invalid = runtime.operations.upload(AnalyzeUpload { title: String::new() }, stream_settings()).unwrap();
    invalid.finish_input();
    let error = invalid.join().await.unwrap_err();
    assert_eq!(error.failure().unwrap().code(), FailureKind::Invalid);

    // Scene 7: Explicit Reload Replaces The Generation And Serves The Same Directory
    drop(progress);
    runtime.plugin.reload_sidecar::<Native>(executable, plugin_settings(), Lanes::Open).await.unwrap();
    let reloaded = runtime.plugin.status(PLUGIN).unwrap();
    assert_eq!(reloaded.state, PluginState::Ready);
    assert_ne!(ready.generation, reloaded.generation);
    assert_eq!(runtime.operations.call(ListDirectories {}).await.unwrap().directories, ["boards"]);

    // Scene 8: Shutdown Reaps The Owned Child And Withdraws Its Operations
    runtime.plugin.stop(PLUGIN).await.unwrap();
    runtime.plugin.stop(PLUGIN).await.unwrap();
    assert!(!runtime.plugin.status(PLUGIN).unwrap().cleanup_pending);
    assert!(runtime.operations.call(ListDirectories {}).await.is_err());
    runtime.stop().await.unwrap();
}
