/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Staged Managed Bundle Replacement

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

use super::{
    Error,
    actor::{Actor, changed, managed, next_activation},
    content::Installation,
    persistence::{SavedMutation, problem},
};
use harmos_protocol::client as wire;
use prost::Message;

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

impl Actor {
    pub(super) async fn replace(&mut self, request: wire::PluginsReplaceRequest) -> Result<wire::PluginsReplaceResponse, Error> {
        if request.request_id.is_empty() {
            return Err(Error::new("identity_required", "Replace Plugin", "supply a nonempty request identity"));
        }
        let encoded = request.encode_to_vec();
        let saved = self
            .registry
            .entries
            .get(&request.name)
            .ok_or_else(|| Error::new("entry_missing", "Replace Plugin", "select an existing managed plugin entry"))?;
        if let Some(original) = saved.replacements.get(&request.request_id) {
            if original.request != encoded {
                return Err(Error::new(
                    "request_conflict",
                    "Replace Plugin",
                    "resend the original payload or choose a new request identity",
                ));
            }
            return wire::PluginsReplaceResponse::decode(original.response.as_slice())
                .map_err(|error| problem("Replay Plugin Replace", error));
        }
        let previous_executable = saved.executable.clone();
        let mut entry = self.entry(&request.name, &request.expected_revision, "Replace Plugin")?;
        if !matches!(entry.placement.as_ref().and_then(|placement| placement.kind.as_ref()), Some(wire::plugin_placement::Kind::Managed(_)))
        {
            return Err(Error::new("managed_required", "Replace Plugin", "select a managed bundle entry"));
        }
        let placement = request
            .placement
            .clone()
            .ok_or_else(|| Error::new("placement_required", "Replace Plugin", "supply the replacement managed placement"))?;
        let Some(wire::plugin_placement::Kind::Managed(bundle)) = placement.kind.as_ref() else {
            return Err(Error::new("managed_required", "Replace Plugin", "supply a managed bundle"));
        };
        let retained = self.registry.creates.len()
            + self.registry.entries.values().map(|entry| entry.settings_receipts.len() + entry.replacements.len()).sum::<usize>();
        if retained >= self.settings.create_receipts.get() {
            return Err(Error::new("receipt_capacity", "Replace Plugin", "configure capacity for retained mutation receipts"));
        }
        let installation = Installation::prepare(&self.store.storage, bundle, &self.settings).await?;
        let mut registry = self.registry.clone();
        entry.placement = Some(placement);
        entry.revision = registry.revision()?;
        entry.activation = next_activation(entry.activation)?;
        entry.process = None;
        entry.failure = None;
        let saved = registry.entries.get_mut(&entry.name).expect("selected entry exists");
        saved.executable = Some(installation.executable().to_owned());
        saved.executable_digest = Some(installation.digest());
        saved.generation = entry.activation;
        let enabled = entry.value.as_ref().is_some_and(|value| value.enabled);
        let prepared = if enabled {
            let placement = managed(saved, &self.settings)?;
            let prepared = (self.stage)(
                entry.name.clone(),
                placement,
                (!saved.settings.is_empty())
                    .then(|| harmos_protocol::v1::SettingValues::decode(saved.settings.as_slice()))
                    .transpose()
                    .map_err(|error| problem("Read Replacement Settings", error))?,
            )
            .await
            .map_err(|error| problem("Stage Plugin Replacement", error))?;
            let description = &prepared.activation.description;
            entry.state = wire::PluginState::Ready as i32;
            entry.catalog_digest = description.catalog.as_ref().expect("admitted description").digest.clone();
            entry.description = Some(wire::PluginDescription {
                id: description.id.clone(),
                version: description.version.clone(),
                started_at: description.started_at,
                catalog: description.catalog.clone(),
                revision: description.revision,
                sdk: description.sdk.clone(),
                sdk_version: description.sdk_version.clone(),
            });
            entry.process = prepared.activation.process_id.map(|id| wire::PluginProcess {
                id,
                executable_digest: prepared.activation.executable_digest.map(|digest| digest.to_vec()).unwrap_or_default(),
                started_at: description.started_at,
            });
            Some(prepared)
        } else {
            entry.state = wire::PluginState::Stopped as i32;
            None
        };
        registry.entries.get_mut(&entry.name).expect("selected entry exists").entry = entry.encode_to_vec();
        super::settings::materialize(&mut registry)?;
        let response = wire::PluginsReplaceResponse {
            entry: Some(entry.clone()),
            receipt: Some(self.receipt(request.request_id.clone())),
        };
        if response.encoded_len() > self.settings.response_bytes.get() {
            return Err(Error::new(
                "response_capacity",
                "Replace Plugin",
                "configure capacity for the complete replacement entry and receipt",
            ));
        }
        registry.entries.get_mut(&entry.name).expect("selected entry exists").replacements.insert(
            request.request_id,
            SavedMutation {
                request: encoded,
                response: response.encode_to_vec(),
            },
        );
        self.store.save(&registry).await?;
        let attached = match prepared.map(|prepared| (self.activate)(prepared)).transpose() {
            Ok(attached) => attached,
            Err(error) => {
                if let Err(restore) = self.store.save(&self.registry).await {
                    installation.commit();
                    return Err(Error::new(
                        "replacement_restore",
                        format!("Restore Plugin After Failed Replacement: {restore}; activation: {error}"),
                        "keep the previous serving bundle and repair the registry backend",
                    ));
                }
                return Err(problem("Activate Plugin Replacement", error));
            }
        };
        installation.commit();
        self.registry = registry;
        self.publish(Some(changed(entry.clone())))?;
        let stopped = self.stop(&entry.name).await;
        if let Some(attached) = attached {
            self.monitor(&entry.name, entry.activation, Ok(Box::pin(async move { Ok(attached) })));
        }
        let cleaned = stopped
            .and_then(|()| previous_executable.as_deref().map_or(Ok(()), |executable| super::content::remove(executable, &self.settings)));
        if let Err(error) = cleaned {
            self.cleanup_failure(&entry.name, error).await?;
        }
        Ok(response)
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
