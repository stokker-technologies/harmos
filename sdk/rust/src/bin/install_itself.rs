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

use super::Value;
use harmos::{Client, ClientSettings, Context, Operation, Plugin, Result, Visibility, v1};
use prost::Message;
use std::num::NonZeroUsize;
use tonic::Request;

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
    let runtime = tokio::runtime::Builder::new_current_thread().enable_all().build()?;
    runtime.block_on(async {
        let channel = tonic::transport::Endpoint::from_shared(arguments[0].clone())
            .map_err(|error| harmos::Error::Invalid(error.to_string()))?
            .connect()
            .await?;
        let bytes = NonZeroUsize::new(16 * 1024 * 1024).unwrap();
        let transfer = harmos::storage::StorageTransferSettings::default();
        let client = Client::new(channel, ClientSettings::for_storage(transfer, bytes)?)?;
        // Install and Start This Codebase's Running Instance
        client
            .plugins
            .create(Request::new(v1::PluginsCreateRequest {
                request_id: "self-install".into(),
                name: "example".into(),
                placement: Some(v1::PluginPlacement {
                    kind: Some(v1::plugin_placement::Kind::Remote(v1::PluginRemote { url: arguments[1].clone() })),
                }),
                value: Some(v1::PluginValue { enabled: true }),
            }))
            .await?;
        let mut watch = client.plugins.watch(Request::new(v1::PluginsWatchRequest { name: "example".into(), at: None })).await?;
        let ready = loop {
            let event = watch.next().await?.ok_or_else(|| harmos::Error::Invalid("Watch Plugins: closed before Ready".into()))?;
            let entries = match event.event {
                Some(v1::plugins_watch_response::Event::Snapshot(snapshot)) => snapshot.entries,
                Some(v1::plugins_watch_response::Event::Change(v1::PluginChange {
                    kind: Some(v1::plugin_change::Kind::Entry(entry)),
                })) => vec![entry],
                _ => Vec::new(),
            };
            if let Some(entry) = entries.into_iter().find(|entry| entry.state == v1::PluginState::Ready as i32) {
                break entry;
            }
        };
        // Invoke Its Public Operation Through the Host
        let contract =
            ready.description.unwrap().catalog.unwrap().operations.into_iter().find(|operation| operation.id == "example.echo").unwrap();
        let mut data = b"hello".to_vec();
        data.resize(transfer.chunk_bytes() as usize, 0);
        let payload = Value { value: data.clone() }.encode_to_vec();
        let response = client
            .invocations
            .invoke(Request::new(v1::InvocationsInvokeRequest {
                operation: contract.id,
                fingerprint: contract.fingerprint,
                payload,
            }))
            .await?
            .into_inner();
        let value = Value::decode(response.payload.as_slice()).map_err(|error| harmos::Error::Invalid(error.to_string()))?;
        if value.value != data {
            return Err(harmos::Error::Invalid("Invoke example.echo: payload changed".into()));
        }
        println!("hello");
        client
            .plugins
            .delete(Request::new(v1::PluginsDeleteRequest {
                name: "example".into(),
                expected_revision: ready.revision,
            }))
            .await?;
        Ok(())
    })
}

/* === Private Functions === */

/* === Tests === */
