/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Install and Invoke This Plugin Through the Host

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::num::NonZeroUsize;

// Third Party Imports
use prost::Message;
use tokio::runtime::Builder;

// Local Imports
use crate::conformance::Value;
use harmos::client::plugin_change;
use harmos::client::plugin_placement;
use harmos::client::plugins_watch_response::Event;
use harmos::client::{PluginChange, PluginPlacement, PluginRemote, PluginState, PluginValue};
use harmos::plugin::{Context, Operation, Plugin};
use harmos::{Client, ClientSettings, Error, Result, StorageTransferSettings, Visibility};
use harmos_conformance::product::client::{PluginsWatchOptions, Root};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

pub fn run(arguments: &[String]) -> Result<()> {
    if arguments == ["serve"] {
        return Plugin::new("example", "1.0.0")
            .operation(Operation::new("example.echo", |_: &Context, value: Value| async { Ok(value) }).visibility(Visibility::Public))
            .remote()
            .serve();
    }
    let runtime = Builder::new_current_thread().enable_all().build()?;
    runtime.block_on(async {
        let bytes = NonZeroUsize::new(16 * 1024 * 1024).unwrap();
        let transfer = StorageTransferSettings::default();
        let connection = Client::connect(arguments[0].clone(), ClientSettings::for_storage(transfer, bytes)?).await?;
        let client = Root::new(connection.transport_handle());
        // Install and Start This Codebase's Running Instance
        let placement = PluginPlacement {
            kind: Some(plugin_placement::Kind::Remote(PluginRemote { url: arguments[1].clone() })),
        };
        client.plugins.create("example", placement, PluginValue { enabled: true }).await?;
        let mut watch = client
            .plugins
            .watch_with(PluginsWatchOptions {
                name: Some("example".into()),
                ..Default::default()
            })
            .await?;
        let ready = loop {
            let event = watch.next().await?.ok_or_else(|| Error::Invalid("Watch Plugins: closed before Ready".into()))?;
            let entries = match event.event {
                Some(Event::Snapshot(snapshot)) => snapshot.entries,
                Some(Event::Change(PluginChange {
                    kind: Some(plugin_change::Kind::Entry(entry)),
                })) => vec![entry],
                _ => Vec::new(),
            };
            if let Some(entry) = entries.into_iter().find(|entry| entry.state == PluginState::Ready as i32) {
                break entry;
            }
        };
        // Invoke Its Public Operation Through the Host
        let contract = ready
            .description
            .unwrap()
            .declarations
            .unwrap()
            .operations
            .into_iter()
            .find(|operation| operation.id == "example.echo")
            .unwrap();
        let mut data = b"hello".to_vec();
        data.resize(transfer.chunk_bytes() as usize, 0);
        let payload = Value { value: data.clone() }.encode_to_vec();
        let response = client.invocations.create(contract.id, contract.fingerprint, payload).await?;
        let value = Value::decode(response.as_slice()).map_err(|error| Error::Invalid(error.to_string()))?;
        if value.value != data {
            return Err(Error::Invalid("Invoke example.echo: payload changed".into()));
        }
        println!("hello");
        client.plugins.delete("example", ready.revision).await?;
        Ok(())
    })
}

/* === Private Functions === */

/* === Tests === */
