/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Typed Plugin Settings Journey

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

use super::support::{self, Backends, Host};
use crate::plugin_grpc::support::Remote;
use harmos_protocol::{
    client::{self, plugins},
    v1::{self, setting_value::Value},
};
use prost::Message;
use prost_reflect::ReflectMessage;

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

fn catalog() -> v1::Catalog {
    let declarations = v1::SettingsDeclarations {
        values: vec![
            v1::SettingDeclaration {
                name: "enabled".into(),
                title: "Enabled".into(),
                description: String::new(),
                default: Some(v1::SettingValue { value: Some(Value::Flag(false)) }),
            },
            v1::SettingDeclaration {
                name: "limit".into(),
                title: "Limit".into(),
                description: String::new(),
                default: Some(v1::SettingValue { value: Some(Value::Integer(7)) }),
            },
        ],
    };
    let descriptor = declarations.descriptor();
    let types = prost_types::FileDescriptorSet {
        file: descriptor.parent_pool().files().map(|file| file.file_descriptor_proto().clone()).collect(),
    };
    let mut catalog = v1::Catalog {
        types: Some(types),
        sections: vec![v1::Section {
            r#type: descriptor.full_name().into(),
            payload: declarations.encode_to_vec(),
        }],
        ..Default::default()
    };
    catalog.digest = harmos_protocol::schema::catalog_digest(&catalog);
    catalog
}

/* === Tests === */

#[tokio::test]
async fn settings_replay_survive_restart_and_delete_with_their_entry() {
    // Observe Each Entry's Reported Declarations.
    let root = tempfile::tempdir().unwrap();
    let backends = Backends::new();
    let remote = Remote::start().await;
    remote.catalog(catalog());
    let mut host = Host::start(root.path(), &backends).await;
    let mut watch = host.client.watch(client::PluginsWatchRequest::default()).await.unwrap().into_inner();
    host.client.create(support::remote("first", &remote.address)).await.unwrap();
    support::state(&mut watch, "first", client::PluginState::Ready, 1).await;
    host.client.create(support::remote("second", &remote.address)).await.unwrap();
    support::state(&mut watch, "second", client::PluginState::Ready, 1).await;
    let mut settings = plugins::settings_client::SettingsClient::new(host.channel.clone());
    let initial = settings.list(plugins::SettingsListRequest { name: "first".into(), at: None }).await.unwrap().into_inner();
    let defaults = initial.value.clone().unwrap();
    assert_eq!(defaults.values["enabled"].value, Some(Value::Flag(false)));
    assert_eq!(defaults.values["limit"].value, Some(Value::Integer(7)));

    // Replace One Complete Value And Replay Its Original Mutation.
    let mut changed = defaults.clone();
    changed.values.get_mut("enabled").unwrap().value = Some(Value::Flag(true));
    changed.values.get_mut("limit").unwrap().value = Some(Value::Integer(11));
    let request = plugins::SettingsSetRequest {
        name: "first".into(),
        value: Some(changed.clone()),
        expected_revision: initial.revision.clone(),
        request_id: "update".into(),
    };
    let applied = settings.set(request.clone()).await.unwrap().into_inner();
    assert!(applied.receipt.as_ref().unwrap().completed);
    assert_ne!(applied.revision, initial.revision);
    let mut reordered = request.clone();
    let values = reordered.value.as_mut().unwrap();
    let entries = values.values.drain().collect::<std::collections::BTreeMap<_, _>>();
    values.values.extend(entries.into_iter().rev());
    assert_eq!(settings.set(reordered).await.unwrap().into_inner(), applied);
    let mut conflicting = request.clone();
    conflicting.value.as_mut().unwrap().values.get_mut("limit").unwrap().value = Some(Value::Integer(13));
    assert!(settings.set(conflicting).await.is_err());
    let mut stale = request.clone();
    stale.request_id = "stale".into();
    assert!(settings.set(stale).await.is_err());
    for (name, value) in [
        (
            "limit",
            Some(v1::SettingValue {
                value: Some(Value::Text("wrong variant".into())),
            }),
        ),
        ("limit", None),
        ("unknown", Some(v1::SettingValue { value: Some(Value::Integer(1)) })),
    ] {
        let mut invalid = request.clone();
        invalid.request_id = format!("invalid-{name}-{}", value.is_some());
        invalid.expected_revision = applied.revision.clone();
        let values = &mut invalid.value.as_mut().unwrap().values;
        if let Some(value) = value {
            values.insert(name.into(), value);
        } else {
            values.remove(name);
        }
        assert!(settings.set(invalid).await.is_err());
    }
    let second = settings.list(plugins::SettingsListRequest { name: "second".into(), at: None }).await.unwrap().into_inner();
    assert_eq!(second.value, Some(defaults.clone()));
    let retained = settings
        .list(plugins::SettingsListRequest {
            name: "first".into(),
            at: initial.resolved_at.clone(),
        })
        .await
        .unwrap()
        .into_inner();
    assert_eq!(retained.value, Some(defaults.clone()));
    host.close().await;

    // Restore Settings And Their Original Receipt From The Durable Registry.
    let mut host = Host::start(root.path(), &backends).await;
    let mut watch = host.client.watch(client::PluginsWatchRequest::default()).await.unwrap().into_inner();
    let ready = support::state(&mut watch, "first", client::PluginState::Ready, 2).await;
    let mut settings = plugins::settings_client::SettingsClient::new(host.channel.clone());
    assert_eq!(settings.set(request).await.unwrap().into_inner(), applied);
    let current = settings.list(plugins::SettingsListRequest { name: "first".into(), at: None }).await.unwrap().into_inner();
    assert_eq!(current.value, Some(changed));

    // Delete Purges Library Values Before The Deletion Watch Is Published.
    host.client
        .delete(client::PluginsDeleteRequest {
            name: "first".into(),
            expected_revision: ready.revision,
        })
        .await
        .unwrap();
    loop {
        let event = watch.message().await.unwrap().unwrap();
        if matches!(event.event, Some(client::plugins_watch_response::Event::Change(client::PluginChange { kind: Some(client::plugin_change::Kind::Deleted(ref name)) })) if name == "first")
        {
            break;
        }
    }
    assert!(
        settings
            .list(plugins::SettingsListRequest {
                name: "first".into(),
                at: current.resolved_at.clone()
            })
            .await
            .is_err()
    );
    let mut reinstall = support::remote("first", &remote.address);
    reinstall.request_id = "reinstall".into();
    host.client.create(reinstall).await.unwrap();
    support::state(&mut watch, "first", client::PluginState::Ready, 1).await;
    assert_eq!(
        settings.list(plugins::SettingsListRequest { name: "first".into(), at: None }).await.unwrap().into_inner().value,
        Some(defaults)
    );
    assert!(
        settings
            .list(plugins::SettingsListRequest {
                name: "first".into(),
                at: current.resolved_at
            })
            .await
            .is_err()
    );
    host.close().await;
}

#[tokio::test]
async fn unmanaged_reported_identity_cannot_select_a_stored_entry() {
    use futures_util::stream;
    use harmos_protocol::plugin::{
        self, host::SettingsListRequest, instance_samples_request as request, instance_samples_response as response,
    };
    use harmos_runtime::{clock::Clock, plugin::grpc::Placement};
    use std::{
        sync::{
            Arc,
            atomic::{AtomicBool, Ordering},
        },
        time::{Duration, SystemTime},
    };
    use tokio::sync::mpsc;

    let root = tempfile::tempdir().unwrap();
    let backends = Backends::new();
    let (clock, controller) = Clock::virtual_at(SystemTime::UNIX_EPOCH);
    let mut host = Host::with_clock(root.path(), &backends, clock).await;
    let victim = Remote::start().await;
    victim.catalog(catalog());
    let mut watch = host.client.watch(client::PluginsWatchRequest::default()).await.unwrap().into_inner();
    host.client.create(support::remote("remote", &victim.address)).await.unwrap();
    support::state(&mut watch, "remote", client::PluginState::Ready, 1).await;
    drop(watch);

    let descriptor = SettingsListRequest::default().descriptor();
    let types = prost_types::FileDescriptorSet {
        file: descriptor.parent_pool().files().map(|file| file.file_descriptor_proto().clone()).collect(),
    };
    let id = "harmos.plugin.host.v1.Settings/List";
    let fingerprint = harmos_protocol::schema::fingerprint(
        id,
        &types,
        "harmos.plugin.host.v1.SettingsListRequest",
        "harmos.plugin.host.v1.SettingsListResponse",
    )
    .unwrap();
    let mut description = v1::Catalog {
        types: Some(types),
        requires: vec![v1::Requirement {
            id: id.into(),
            fingerprint: fingerprint.clone(),
        }],
        streams: vec![v1::StreamDeclaration {
            id: "probe".into(),
            row: "harmos.plugin.host.v1.SettingsListRequest".into(),
            interval: Some(prost_types::Duration { seconds: 1, nanos: 0 }),
            ..Default::default()
        }],
        ..Default::default()
    };
    description.digest = harmos_protocol::schema::catalog_digest(&description);
    let attacker = Remote::start().await;
    // Both Instances Report "remote"; Only The Manager Selects The Entry Identity.
    attacker.catalog(description);
    let refused = Arc::new(AtomicBool::new(false));
    let observed = refused.clone();
    attacker.samples(move |mut input| {
        let (output, receiver) = mpsc::channel(1);
        let fingerprint = fingerprint.clone();
        let observed = observed.clone();
        tokio::spawn(async move {
            assert!(matches!(input.message().await.unwrap().unwrap().frame, Some(request::Frame::Open(_))));
            assert!(matches!(input.message().await.unwrap().unwrap().frame, Some(request::Frame::Tick(_))));
            output
                .send(Ok(plugin::InstanceSamplesResponse {
                    samples: vec![],
                    frame: Some(response::Frame::Child(v1::ChildRequest {
                        call: 1,
                        operation: id.into(),
                        fingerprint,
                        payload: SettingsListRequest::default().encode_to_vec(),
                        ..Default::default()
                    })),
                }))
                .await
                .unwrap();
            let Some(request::Frame::Child(reply)) = input.message().await.unwrap().unwrap().frame else {
                panic!("Expected Child Reply");
            };
            let Some(v1::child_response::Result::Failure(failure)) = reply.result else {
                panic!("Expected Refusal");
            };
            assert_eq!(reply.call, 1);
            assert_eq!(failure.classification(), v1::Classification::Refused);
            observed.store(true, Ordering::SeqCst);
        });
        Box::pin(stream::unfold(receiver, |mut receiver| async { receiver.recv().await.map(|item| (item, receiver)) }))
    });
    let attached = host.runtime.attach_grpc(Placement::Remote(attacker.address.clone()), super::super::support::settings()).await.unwrap();
    assert!(attached.activation().unwrap().entry_name.is_none());
    controller.advance_until(Duration::from_secs(2), || refused.load(Ordering::SeqCst)).await.unwrap();
    attached.stop("Story Complete").await.unwrap();
    host.close().await;
}
