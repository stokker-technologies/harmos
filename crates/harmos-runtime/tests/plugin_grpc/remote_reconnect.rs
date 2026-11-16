/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Remote Plugin Connection Replacement

/* === Imports === */

use harmos_protocol::plugin::InstanceSamplesResponse;

// Standard Library Imports

// Third Party Imports
use futures_util::stream;
use prost::Message as _;
use prost_reflect::ReflectMessage as _;
use tokio::sync::mpsc;
use tonic::Status;

// Local Imports
use super::support::{self, Manual, Remote};
use harmos_protocol::model;
use harmos_runtime::{
    clock::{Clock, Source as _},
    plugin::grpc::{Error, Placement},
};

/* === Private Functions === */

/// Declare One Integer Setting, or None When the Peer Has Dropped It.
fn settings_catalog(declared: bool) -> model::Declarations {
    let values = if declared {
        vec![model::SettingDeclaration {
            name: "limit".into(),
            title: "Limit".into(),
            description: String::new(),
            default: Some(model::SettingValue {
                value: Some(model::setting_value::Value::Integer(7)),
            }),
        }]
    } else {
        Vec::new()
    };
    let declarations = model::SettingsDeclarations { values };
    let descriptor = declarations.descriptor();
    let types = prost_types::FileDescriptorSet {
        file: descriptor.parent_pool().files().map(|file| file.file_descriptor_proto().clone()).collect(),
    };
    let mut catalog = model::Declarations {
        types: Some(types),
        sections: vec![model::Section {
            r#type: descriptor.full_name().into(),
            payload: declarations.encode_to_vec(),
        }],
        ..Default::default()
    };
    catalog.digest = harmos_protocol::schema::declarations_digest(&catalog);
    catalog
}

/// A Device-Stream Peer Whose Demands End When the Host Half-Closes Them.
async fn device() -> Remote {
    let remote = Remote::start().await;
    let mut catalog = model::Declarations {
        types: Some(harmos_protocol::clock::declarations_types()),
        streams: vec![model::StreamDeclaration {
            id: "probe".into(),
            row: "harmos.host.v1.TimeGetResponse".into(),
            ..Default::default()
        }],
        ..Default::default()
    };
    catalog.digest = harmos_protocol::schema::declarations_digest(&catalog);
    remote.declarations(catalog);
    remote.samples(|mut input| {
        let (output, receiver) = mpsc::channel::<Result<InstanceSamplesResponse, Status>>(1);
        tokio::spawn(async move {
            while input.message().await.ok().flatten().is_some() {}
            drop(output);
        });
        Box::pin(stream::unfold(receiver, |mut receiver| async { receiver.recv().await.map(|item| (item, receiver)) }))
    });
    remote
}

/* === Tests === */

#[tokio::test]
async fn remote_reconnect_exposes_process_identity_and_retires_old_handles() {
    // Connect A Remote Plugin Using The Host Clock
    let source = Manual::new();
    let runtime = support::runtime(Some(Clock::with_clock(source.clone()))).await;
    let remote = Remote::start().await;
    let attached = runtime.attach_grpc(Placement::Remote(remote.address.clone()), support::settings()).await.unwrap();
    let first = attached.activation().unwrap();
    let mut status = attached.status();

    // Hold A Call Across The Drop
    let entered = remote.hold_description();
    let retained = first.clone();
    let inflight = tokio::spawn(async move { retained.get().await });
    entered.await.unwrap();

    // Reconnect To The Same Process Without Reusing The Old Connection
    remote.drop_connection(100).await;
    support::offline(&mut status).await;
    assert!(!first.is_connected());
    assert!(inflight.await.unwrap().is_err());
    source.advance_next().await;
    let second = support::activation(&mut status, 2).await;
    assert_eq!(second.description.started_at, first.description.started_at);
    assert_eq!(second.previous_started_at, first.description.started_at);
    assert_eq!(second.description.declarations, first.description.declarations);
    assert!(first.get().await.is_err());
    assert!(second.get().await.is_ok());

    // A Restart Exposes Both Start Times In A Fresh Activation
    remote.drop_connection(200).await;
    support::offline(&mut status).await;
    source.advance_next().await;
    let third = support::activation(&mut status, 3).await;
    assert_ne!(third.description.started_at, second.description.started_at);
    assert_eq!(third.previous_started_at, second.description.started_at);
    assert!(second.get().await.is_err());

    // Removing A Remote Plugin Calls Stop And Leaves Its Server Running
    attached.stop("Removed").await.unwrap();
    assert_eq!(remote.stops(), 1);
    assert_eq!(remote.get().await.started_at, third.description.started_at);
    runtime.shutdown().await.unwrap();
}

#[tokio::test]
async fn a_reconnect_refused_by_its_settings_keeps_reconnecting() {
    // Attach a Remote Plugin That Declares One Setting.
    let source = Manual::new();
    let settings = support::settings();
    let runtime = support::runtime(Some(Clock::with_clock(source.clone()))).await;
    let remote = Remote::start().await;
    remote.declarations(settings_catalog(true));
    let attached = runtime.attach_grpc(Placement::Remote(remote.address.clone()), settings.clone()).await.unwrap();
    let first = attached.activation().unwrap();
    let mut status = attached.status();

    // The Restarted Peer No Longer Declares It, So the Retained Value Cannot Be Selected.
    remote.declarations(settings_catalog(false));
    remote.drop_connection(200).await;
    support::offline(&mut status).await;
    source.advance_next().await;
    let retry = source.read().instant + settings.reconnect_delay;
    tokio::select! {
        biased;
        () = support::ended(&mut status) => panic!("Keep Reconnecting After a Settings Refusal"),
        () = source.registered_at(retry) => {},
    }
    let failure = status.borrow().failure.clone().expect("Report the Settings Refusal");
    assert!(matches!(failure.as_ref(), Error::Invalid(_)), "Refuse as Invalid: {failure}");

    // Restoring the Declaration Lets the Next Attempt Admit the Retained Value.
    remote.declarations(settings_catalog(true));
    source.advance_next().await;
    let second = support::activation(&mut status, 2).await;
    assert_eq!(second.settings, first.settings);
    attached.stop("Story Complete").await.unwrap();
    runtime.shutdown().await.unwrap();
}

#[tokio::test]
async fn a_reconnect_whose_samples_never_answer_is_bounded_and_stoppable() {
    // Attach a Device-Stream Peer, Then Withhold Samples From Its Next Connection.
    let source = Manual::new();
    let settings = support::settings();
    let runtime = support::runtime(Some(Clock::with_clock(source.clone()))).await;
    let remote = device().await;
    let attached = runtime.attach_grpc(Placement::Remote(remote.address.clone()), settings.clone()).await.unwrap();
    let mut status = attached.status();
    remote.drop_connection(200).await;
    support::offline(&mut status).await;
    let (entered, abandoned) = remote.hold_samples();
    source.advance_next().await;
    let deadline = source.read().instant + settings.connect_timeout;
    support::bounded(settings.connect_timeout, entered).await.unwrap();

    // The Attempt's Connect Timeout Bounds Opening Its Publications.
    assert!(source.timer_at(deadline), "Bound Opening Publications by the Attempt Deadline");
    source.advance_by(settings.connect_timeout);
    support::bounded(settings.stop_grace, abandoned).await.unwrap();
    let retry = source.read().instant + settings.reconnect_delay;
    tokio::select! {
        biased;
        () = support::ended(&mut status) => panic!("Keep Reconnecting After an Unanswered Open"),
        () = source.registered_at(retry) => {},
    }
    let failure = status.borrow().failure.clone().expect("Report the Unanswered Open");
    assert!(matches!(failure.as_ref(), Error::Timeout(_)), "Report the Timeout: {failure}");

    // Stop Interrupts the Next Unanswered Open and Releases Its Demand.
    let (entered, abandoned) = remote.hold_samples();
    source.advance_next().await;
    support::bounded(settings.connect_timeout, entered).await.unwrap();
    support::bounded(settings.stop_grace, attached.stop("Story Complete")).await.unwrap();
    support::bounded(settings.stop_grace, abandoned).await.unwrap();
    assert_eq!(remote.stops(), 0, "Never Stop a Peer Whose Activation Was Not Admitted");
    runtime.shutdown().await.unwrap();
}
