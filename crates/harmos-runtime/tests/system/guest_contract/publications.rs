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
use std::pin::Pin;
// Third Party Imports
use futures_core::Stream;
// Local Imports
use super::lifecycle::{Lifecycle, Manager};
use super::{runtime, sandbox as wire};
use harmos_runtime::Message;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

/* === Private Functions === */

async fn values(stream: &harmos_runtime::streams::PluginStream) -> Vec<String> {
    stream.query(..).await.unwrap().map(|row| wire::Echoed::decode(row.value.payload.as_slice()).unwrap().said).collect()
}

#[tokio::test]
async fn publications_survive_unload() {
    let runtime = runtime().await;
    let mut lifecycle = Lifecycle::sandbox(&runtime);
    let mut manager = Manager::new(lifecycle.candidate("publications").await).unwrap();
    manager.initialize(&mut lifecycle).await.unwrap();
    let stream = runtime.streams.publication("progress", 1).unwrap();
    assert_eq!(stream.definition().owner(), Some("sandbox-guest"));
    assert_eq!(stream.definition().schema().unwrap().root().unwrap().fields()[0].name, "said");
    runtime.operations.call(wire::Echo { say: "publish:route".into() }).await.unwrap();
    manager.shutdown(&mut lifecycle).await.unwrap();
    assert_eq!(values(&stream).await, ["initialize", "route"], "shutdown cannot admit another publication after the fence");
    drop(manager);
    let mut replacement = Manager::new(lifecycle.candidate("idle").await).unwrap();
    replacement.initialize(&mut lifecycle).await.unwrap();
    runtime.operations.call(wire::Echo { say: "publish:replacement".into() }).await.unwrap();
    replacement.shutdown(&mut lifecycle).await.unwrap();
    assert_eq!(values(&stream).await, ["initialize", "route", "replacement"]);
    drop((replacement, lifecycle));
    runtime.stop().await.unwrap();
}

#[tokio::test]
async fn replacement_preserves_owner() {
    let runtime = runtime().await;
    let mut lifecycle = Lifecycle::sandbox(&runtime);
    let mut manager = Manager::new(lifecycle.candidate("idle").await).unwrap();
    manager.initialize(&mut lifecycle).await.unwrap();
    let stream = runtime.streams.publication("progress", 1).unwrap();
    let candidate = lifecycle.candidate("replacement").await;
    let plan = manager.plan_reload(candidate).unwrap();
    runtime.operations.call(wire::Echo { say: "publish:original".into() }).await.unwrap();
    assert_eq!(values(&stream).await, ["original"]);
    manager.apply_reload(plan, &mut lifecycle).await.unwrap();
    runtime.operations.call(wire::Echo { say: "publish:replacement".into() }).await.unwrap();
    assert_eq!(values(&stream).await, ["original", "replacement"]);
    manager.shutdown(&mut lifecycle).await.unwrap();
    drop((manager, lifecycle));
    runtime.stop().await.unwrap();
}

#[tokio::test]
async fn initialization_aborts_staging() {
    let runtime = runtime().await;
    let mut lifecycle = Lifecycle::sandbox(&runtime);
    let mut manager = Manager::new(lifecycle.candidate("failed-publications").await).unwrap();
    assert!(manager.initialize(&mut lifecycle).await.is_err());
    manager.shutdown(&mut lifecycle).await.unwrap();
    if let Some(stream) = runtime.streams.publication("progress", 1) {
        assert!(values(&stream).await.is_empty(), "an aborted activation cannot expose its staged row");
    }
    manager.reload(lifecycle.candidate("idle").await, &mut lifecycle).await.unwrap();
    let stream = runtime.streams.publication("progress", 1).unwrap();
    runtime.operations.call(wire::Echo { say: "publish:replacement".into() }).await.unwrap();
    assert_eq!(values(&stream).await, ["replacement"]);
    manager.shutdown(&mut lifecycle).await.unwrap();
    drop((manager, lifecycle));
    runtime.stop().await.unwrap();
}

#[tokio::test]
async fn bounded_window_gaps() {
    let runtime = runtime().await;
    let mut lifecycle = Lifecycle::sandbox(&runtime);
    let mut manager = Manager::new(lifecycle.candidate("idle").await).unwrap();
    manager.initialize(&mut lifecycle).await.unwrap();
    let stream = runtime.streams.publication("progress", 1).unwrap();
    let mut rows = stream.watch();
    runtime.operations.call(wire::Echo { say: "publish-many".into() }).await.unwrap();
    let gap = std::future::poll_fn(|cx| Pin::new(&mut rows).poll_next(cx)).await;
    assert_eq!(gap, Some(harmos_runtime::streams::Item::Gap { missed: 4 }));
    let mut retained = Vec::new();
    for _ in 0..4 {
        let row = std::future::poll_fn(|cx| Pin::new(&mut rows).poll_next(cx)).await;
        let Some(harmos_runtime::streams::Item::Row(row)) = row else { panic!("retained row expected") };
        retained.push(wire::Echoed::decode(row.value.payload.as_slice()).unwrap().said);
    }
    assert_eq!(retained, ["4", "5", "6", "7"]);
    manager.shutdown(&mut lifecycle).await.unwrap();
    drop((manager, lifecycle));
    runtime.stop().await.unwrap();
}

#[tokio::test]
async fn saved_context_generation() {
    let runtime = runtime().await;
    let mut lifecycle = Lifecycle::sandbox(&runtime);
    let mut manager = Manager::new(lifecycle.candidate("idle").await).unwrap();
    manager.initialize(&mut lifecycle).await.unwrap();
    let stream = runtime.streams.publication("progress", 1).unwrap();
    let published = runtime.operations.call(wire::Echo { say: "saved-publish:original".into() }).await.unwrap();
    assert_eq!(published.said, "published original");
    assert_eq!(values(&stream).await, ["original"]);
    let dependency = runtime.operations.call(wire::Echo { say: "saved-dependency:original".into() }).await.unwrap();
    assert_eq!(dependency.said, "provider:original");
    manager.shutdown(&mut lifecycle).await.unwrap();
    drop((manager, lifecycle));
    runtime.stop().await.unwrap();
}

/* === Tests === */
