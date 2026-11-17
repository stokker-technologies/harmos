/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Typed Plugin Facade Stories

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::{
    num::NonZeroUsize,
    time::{Duration, SystemTime},
};

// Third Party Imports

// Local Imports
use super::{
    invoke_support::HostFixture,
    support::{self, Remote},
};
use harmos_runtime::{
    clock::Clock,
    facade::plugins::{BindRequest, InspectRequest, LoadRequest, Placement, SettleRequest, StartSamplesRequest, StopSamplesRequest},
    facade::time::TimeGetRequest,
    operations::CallContext,
};

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
async fn plugin_activation_through_operations() {
    // Load One Generation
    let runtime = support::runtime(None).await;
    let remote = Remote::start().await;
    let caller = runtime.for_call(&CallContext::default());
    let plugin = caller
        .plugins()
        .load(LoadRequest {
            placement: Placement::Remote(remote.address.clone()),
            settings: support::settings(),
        })
        .await
        .unwrap();

    // Bind an Independent Clock
    let (clock, _) = Clock::virtual_at(SystemTime::UNIX_EPOCH + Duration::from_secs(100));
    let binding = caller
        .plugins()
        .bind(BindRequest {
            plugin: plugin.clone(),
            scope: "facade/unit".into(),
            clock: clock.clone(),
        })
        .await
        .unwrap();
    let now = binding.for_call().time().get(TimeGetRequest {}).await.unwrap();
    assert_eq!(now.now.unwrap().seconds, 100);

    // Refuse Another Runtime's Handle
    let other = support::runtime(None).await;
    assert!(
        other
            .for_call(&CallContext::default())
            .plugins()
            .bind(BindRequest {
                plugin: plugin.clone(),
                scope: "other".into(),
                clock: clock.clone()
            })
            .await
            .is_err()
    );
    other.shutdown().await.unwrap();

    // Settle Every Clone and Fence the Retired Generation
    caller.plugins().settle(SettleRequest::Binding(binding.clone())).await.unwrap();
    assert!(binding.for_call().time().get(TimeGetRequest {}).await.is_err());
    caller.plugins().settle(SettleRequest::Plugin(plugin.clone())).await.unwrap();
    assert!(caller.plugins().bind(BindRequest { plugin, scope: "retired".into(), clock }).await.is_err());
    runtime.shutdown().await.unwrap();
}

#[tokio::test]
async fn plugin_inspection_is_metadata_only_and_detaches() {
    let runtime = support::runtime(None).await;
    let remote = Remote::start().await;
    let metadata = runtime
        .for_call(&CallContext::default())
        .plugins()
        .inspect(InspectRequest {
            placement: Placement::Remote(remote.address.clone()),
            settings: support::settings(),
            metadata_bytes: NonZeroUsize::new(1024 * 1024).unwrap(),
        })
        .await
        .unwrap();
    assert_eq!(metadata.metadata.id, "remote");
    assert!(runtime.operations.definition("fixture.echo").is_err());
    assert_eq!(remote.stops(), 0);
    assert_eq!(remote.get().await.id, "remote");
    runtime.shutdown().await.unwrap();
}

#[tokio::test]
async fn plugin_inspection_refuses_oversized_and_invalid_metadata() {
    let runtime = support::runtime(None).await;
    let remote = Remote::start().await;
    let caller = runtime.for_call(&CallContext::default());
    assert!(
        caller
            .plugins()
            .inspect(InspectRequest {
                placement: Placement::Remote(remote.address.clone()),
                settings: support::settings(),
                metadata_bytes: NonZeroUsize::new(1).unwrap()
            })
            .await
            .is_err()
    );
    remote.declarations(Default::default());
    assert!(
        caller
            .plugins()
            .inspect(InspectRequest {
                placement: Placement::Remote(remote.address.clone()),
                settings: support::settings(),
                metadata_bytes: NonZeroUsize::new(1024 * 1024).unwrap()
            })
            .await
            .is_err()
    );
    assert_eq!(remote.stops(), 0);
    runtime.shutdown().await.unwrap();
}

#[tokio::test]
async fn plugin_inspection_cancellation_settles_the_connection() {
    let runtime = support::runtime(None).await;
    let remote = Remote::start().await;
    let (entered, release) = remote.delay_description();
    let caller = runtime.handle().for_call(&CallContext::default());
    let placement = Placement::Remote(remote.address.clone());
    let inspecting = tokio::spawn(async move {
        caller
            .plugins()
            .inspect(InspectRequest {
                placement,
                settings: support::settings(),
                metadata_bytes: NonZeroUsize::new(1024 * 1024).unwrap(),
            })
            .await
    });
    entered.await.unwrap();
    inspecting.abort();
    assert!(inspecting.await.err().unwrap().is_cancelled());
    release.send(()).unwrap();
    runtime.shutdown().await.unwrap();
    assert_eq!(remote.get().await.id, "remote");
    assert_eq!(remote.stops(), 0);
}

#[tokio::test]
async fn plugin_inspection_failed_connect_settles_the_owner() {
    let runtime = support::runtime(None).await;
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let placement = Placement::Remote(listener.local_addr().unwrap().to_string());
    let peer = tokio::spawn(async move {
        let (socket, _) = listener.accept().await.unwrap();
        drop(socket);
    });
    assert!(
        runtime
            .for_call(&CallContext::default())
            .plugins()
            .inspect(InspectRequest {
                placement,
                settings: support::settings(),
                metadata_bytes: NonZeroUsize::new(1024 * 1024).unwrap()
            })
            .await
            .is_err()
    );
    peer.await.unwrap();
    runtime.shutdown().await.unwrap();
}

#[tokio::test]
async fn managed_plugin_inspection_settles_without_admission() {
    let host = HostFixture::new().await;
    let placement = host.managed_sdk("interval");
    host.runtime
        .for_call(&CallContext::default())
        .plugins()
        .inspect(InspectRequest {
            placement,
            settings: support::settings(),
            metadata_bytes: NonZeroUsize::new(1024 * 1024).unwrap(),
        })
        .await
        .unwrap();
    assert!(host.runtime.operations.definition("fixture.echo").is_err());
    host.runtime.shutdown().await.unwrap();
}

#[tokio::test]
async fn sampling_through_operations() {
    let host = HostFixture::new().await;
    let sdk = host.sdk("scoped-publications").await;
    let caller = host.runtime.for_call(&CallContext::default());
    let plugin = caller
        .plugins()
        .load(LoadRequest {
            placement: Placement::Remote(sdk.address.clone()),
            settings: support::settings(),
        })
        .await
        .unwrap();
    let (clock, controller) = Clock::virtual_at(SystemTime::UNIX_EPOCH);
    let binding = caller
        .plugins()
        .bind(BindRequest {
            plugin: plugin.clone(),
            scope: "facade/samples".into(),
            clock,
        })
        .await
        .unwrap();
    let samples = caller
        .plugins()
        .start_samples(StartSamplesRequest {
            binding: binding.clone(),
            streams: vec!["fixture.scoped_samples".into()],
        })
        .await
        .unwrap();
    controller.advance_until(Duration::from_secs(5), || !host.rows().is_empty()).await.unwrap();
    assert!(host.rows().iter().all(|row| row.context.as_ref().is_some_and(|value| value.contains("facade/samples"))));
    caller.plugins().stop_samples(StopSamplesRequest { samples: samples.clone() }).await.unwrap();
    let next = caller
        .plugins()
        .start_samples(StartSamplesRequest {
            binding: binding.clone(),
            streams: vec!["fixture.scoped_samples".into()],
        })
        .await
        .unwrap();
    assert!(caller.plugins().stop_samples(StopSamplesRequest { samples }).await.is_err());
    caller.plugins().stop_samples(StopSamplesRequest { samples: next }).await.unwrap();
    caller.plugins().settle(SettleRequest::Binding(binding)).await.unwrap();
    caller.plugins().settle(SettleRequest::Plugin(plugin)).await.unwrap();
    host.runtime.shutdown().await.unwrap();
}
