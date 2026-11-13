/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Retain Published Guest Rows

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

use super::{contract::*, support};
use std::{sync::Arc, time::SystemTime};

// Standard Library Imports

// Third Party Imports

// Local Imports

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

/* === Private Functions === */

async fn values(stream: &harmos_runtime::streams::PublishedStream) -> Vec<String> {
    stream.query(..).retained().await.unwrap().map(|row| <String as harmos_runtime::Message>::decode(&row.value.payload).unwrap()).collect()
}

/* === Tests === */

#[tokio::test]
async fn guest_publications_abort_staging_and_survive_replacement() {
    let _report = crate::report::system();
    // Refuse Initialization After a Staged Publication
    let (clock, _) = harmos_runtime::clock::Clock::virtual_at(SystemTime::UNIX_EPOCH);
    let lane = harmos_runtime::operations::Lane::default();
    let runtime = support::runtime(Arc::new(support::ProviderOwner::default()), lane, clock).await;
    let host = support::host(&runtime);
    runtime.plugin.register_guest(host.load(&support::sandbox()).unwrap(), support::settings("failed-publications"), lane).await.unwrap();
    assert!(runtime.plugin.initialize().await.is_err());
    runtime.plugin.stop("sandbox_guest").await.unwrap();
    if let Some(stream) = runtime.streams.current_publication("progress") {
        assert!(values(&stream).await.is_empty());
    }

    // Activate and Publish Through the Original Generation's Lease
    runtime.plugin.reload_guest(host.load(&support::sandbox()).unwrap(), support::settings("publications"), lane).await.unwrap();
    let stream = runtime.streams.current_publication("progress").unwrap();
    assert_eq!(stream.definition().owner(), Some("sandbox_guest"));
    assert_eq!(stream.definition().schema().root().unwrap().fields()[0].name, "said");
    support::call(&runtime.operations, EchoOperation { say: "publish:route".into() }).await.unwrap();
    support::call(&runtime.operations, EchoOperation { say: "saved-publish:saved".into() }).await.unwrap();
    assert_eq!(values(&stream).await, ["initialize", "route", "saved"]);

    // Retire the Generation Before Shutdown Can Publish Again
    runtime.plugin.stop("sandbox_guest").await.unwrap();
    assert_eq!(values(&stream).await, ["initialize", "route", "saved"]);

    // Reuse the Stream's Durable Identity in a Replacement Activation
    runtime.plugin.reload_guest(host.load(&support::sandbox()).unwrap(), support::settings("normal"), lane).await.unwrap();
    support::call(&runtime.operations, EchoOperation { say: "publish:replacement".into() }).await.unwrap();
    assert_eq!(values(&stream).await, ["initialize", "route", "saved", "replacement"]);
    support::call(&runtime.operations, EchoOperation { say: "publish-many".into() }).await.unwrap();
    assert_eq!(values(&stream).await, ["4", "5", "6", "7"]);
    runtime.plugin.stop("sandbox_guest").await.unwrap();
    assert_eq!(values(&stream).await, ["4", "5", "6", "7"]);
    runtime.shutdown().await.unwrap();
}
