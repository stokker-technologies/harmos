/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Publication Demand Reopens After Reconnect

/* === Imports === */

// Standard Library Imports
use std::time::{Duration, SystemTime};

// Third Party Imports

// Local Imports
use super::{invoke_support, support};

#[cfg(feature = "plugin-guest")]
use harmos_protocol::plugin::InstanceGetResponse;

use harmos_runtime::clock::Clock;
use harmos_runtime::plugin::grpc::Placement;
use harmos_runtime::storage::Cursor;

#[cfg(feature = "plugin-guest")]
use harmos_runtime::guest::{
    PluginArtifact, PluginBackend, PluginContributions, PluginDefinition, PluginDefinitions, PluginEndpoint, PluginLifecycle,
    PluginLifecycleAuthority, PluginManager, PluginTransition, Router,
};

#[cfg(feature = "plugin-guest")]
use harmos_runtime::Result;

/* === Structs === */

#[cfg(feature = "plugin-guest")]
struct Guest;

/* === Implementations === */

#[cfg(feature = "plugin-guest")]
impl Guest {
    fn manager(description: &InstanceGetResponse) -> PluginManager<Option<PluginEndpoint>, ()> {
        let definition = PluginDefinition {
            id: description.id.clone(),
            version: description.version.parse().unwrap(),
            artifact: PluginArtifact::Guest { sha256: [0; 32] },
            requires: Vec::new(),
            configuration: b"null".to_vec(),
            contributions: PluginContributions::default(),
        };
        let definitions = PluginDefinitions::new([definition], []).unwrap();
        PluginManager::new(definitions).unwrap()
    }
}

#[cfg(feature = "plugin-guest")]
impl PluginLifecycle for Guest {
    type Guest = Option<PluginEndpoint>;

    type Embedded = ();

    fn prepare(&mut self, _: &PluginDefinition) -> Result<PluginBackend<Self::Guest, Self::Embedded>> {
        Ok(PluginBackend::Guest(None))
    }

    async fn fence(&mut self, _: PluginTransition, _: &[PluginDefinition]) -> Result<()> {
        Ok(())
    }

    async fn start(
        &mut self, definition: &PluginDefinition, router: &Router, _: &PluginLifecycleAuthority<'_>,
        backend: &mut PluginBackend<Self::Guest, Self::Embedded>,
    ) -> Result<()> {
        let PluginBackend::Guest(endpoint) = backend else { unreachable!() };
        *endpoint = Some(router.enter(&definition.id).unwrap());
        Ok(())
    }

    async fn stop(
        _: &PluginDefinition, _: &PluginLifecycleAuthority<'_>, backend: &mut PluginBackend<Self::Guest, Self::Embedded>,
    ) -> Result<()> {
        let PluginBackend::Guest(endpoint) = backend else { unreachable!() };
        endpoint.take();
        Ok(())
    }
}

/* === Tests === */

#[cfg(feature = "plugin-guest")]
#[tokio::test]
async fn guest_and_grpc_activations_share_generations_across_fresh_attachments() {
    // Admit a Guest Generation.
    let remote = support::Remote::start().await;
    let description = remote.get().await;
    let mut manager = Guest::manager(&description);
    manager.initialize(&mut Guest).await.unwrap();
    let first = manager.status(&description.id).unwrap().generation.unwrap().get();
    manager.shutdown(&mut Guest).await.unwrap();

    // Admit the Same Identity over gRPC.
    let runtime = support::runtime(None).await;
    let attached = runtime.attach_grpc(Placement::Remote(remote.address.clone()), support::settings()).await.unwrap();
    let second = attached.activation().unwrap();
    assert_eq!(second.description.id, description.id);
    assert!(second.generation > first);
    attached.stop("Generation Retired").await.unwrap();

    // Restart the Guest Identity.
    manager.initialize(&mut Guest).await.unwrap();
    let third = manager.status(&description.id).unwrap().generation.unwrap().get();
    assert!(third > second.generation);
    manager.shutdown(&mut Guest).await.unwrap();

    // Admit a Fresh gRPC Attachment without Reusing Its Counter as Identity.
    let attached = runtime.attach_grpc(Placement::Remote(remote.address.clone()), support::settings()).await.unwrap();
    let fourth = attached.activation().unwrap();
    assert_eq!(fourth.description.id, description.id);
    assert!(fourth.generation > third);
    assert_eq!(fourth.number, second.number);
    attached.stop("Story Complete").await.unwrap();
    runtime.stop().await.unwrap();
}

#[tokio::test]
async fn publish_reopens_with_a_fresh_sequence_after_connection_loss() {
    // Capture One Interval Answer
    let (clock, controller) = Clock::virtual_at(SystemTime::UNIX_EPOCH);
    let host = invoke_support::HostFixture::with_clock(Some(clock)).await;
    *host.signals.publications.lock().unwrap() = Some(Vec::new());
    let sdk = host.sdk("interval").await;
    let attached = host.runtime.attach_grpc(Placement::Remote(sdk.address.clone()), support::settings()).await.unwrap();
    let _watching = host.watch("fixture.interval");
    let mut status = attached.status();
    controller.advance_until(Duration::from_secs(10), || host.rows().len() == 1).await.unwrap();
    let old = attached.activation().unwrap();
    let original = host.signals.publications.lock().unwrap().as_ref().unwrap().first().unwrap().clone();
    assert_eq!(original.1.generation(), Some(old.generation));
    assert_eq!(original.1.owner(), original.0.owner().unwrap());
    assert_eq!(original.1.schema(), original.0.version());
    let mut feed = host.runtime.streams.follow(Cursor::ORIGIN);
    let delayed = feed.next().await.unwrap();
    drop(feed);

    // Interleave a Guest Admission for the Same Identity.
    #[cfg(feature = "plugin-guest")]
    let guest_generation = {
        let mut manager = Guest::manager(&old.description);
        manager.initialize(&mut Guest).await.unwrap();
        let generation = manager.status(&old.description.id).unwrap().generation.unwrap().get();
        assert!(generation > old.generation);
        manager.shutdown(&mut Guest).await.unwrap();
        generation
    };

    // Drop The Physical Connection And Advance Its Reconnect Timer
    sdk.disconnect().await;
    support::offline(&mut status).await;
    controller
        .advance_until(Duration::from_secs(10), || attached.activation().is_some_and(|activation| activation.number > old.number))
        .await
        .unwrap();
    assert!(!old.is_connected());

    // A Reopened Demand Starts At Sequence One
    controller.advance_until(Duration::from_secs(10), || host.rows().len() >= 2).await.unwrap();
    assert_eq!(
        host.samples().into_iter().map(|(_, row)| row).collect::<Vec<_>>(),
        [serde_json::json!({"value": 1_u64.to_le_bytes().to_vec()}), serde_json::json!({"value": 1_u64.to_le_bytes().to_vec()})]
    );
    assert_eq!(host.rows().iter().map(|row| row.published_unix_micros).collect::<Vec<_>>(), [Some(1_000_000), Some(3_000_000)]);
    let restarted = host.signals.publications.lock().unwrap().as_ref().unwrap().last().unwrap().clone();
    let current = attached.activation().unwrap();
    assert_eq!(restarted.1.generation(), Some(current.generation));
    assert!(current.generation > old.generation);

    #[cfg(feature = "plugin-guest")]
    assert!(current.generation > guest_generation);

    assert_eq!(restarted.1.owner(), original.1.owner());
    assert_eq!(restarted.1.schema(), restarted.0.version());
    assert_eq!(original.1.generation(), Some(old.generation));
    let (definition, _, delivered) = delayed.publications().find(|(definition, _, _)| definition.id() == original.0.id()).unwrap();
    assert_eq!(delivered.value.owner(), original.1.owner());
    assert_eq!(delivered.value.generation(), Some(old.generation));
    assert_eq!(delivered.value.schema(), original.1.schema());
    assert_eq!(definition.schema(), original.0.schema());
    attached.stop("Story Complete").await.unwrap();
    host.runtime.stop().await.unwrap();
}
