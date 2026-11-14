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
