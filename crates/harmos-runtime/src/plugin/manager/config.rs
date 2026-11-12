/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Plugin Manager Policy

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::{num::NonZeroUsize, path::PathBuf};
// Third Party Imports
use prost::Message;
// Local Imports
use super::Error;
use crate::{bundle::Limits, plugin::grpc::PlacementSettings};
use harmos_protocol::{v1 as wire, v1};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

#[derive(Clone, Debug)]
pub struct PluginManagerSettings {
    pub namespace: String,
    pub directory: PathBuf,
    pub target: String,
    pub placement: PlacementSettings,
    pub bundle: Limits,
    pub entries: NonZeroUsize,
    pub create_receipts: NonZeroUsize,
    pub registry_bytes: NonZeroUsize,
    pub command_capacity: NonZeroUsize,
    pub retained_cuts: NonZeroUsize,
    pub page_tokens: NonZeroUsize,
    pub watch_capacity: NonZeroUsize,
    pub page_size: NonZeroUsize,
    pub response_bytes: NonZeroUsize,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl PluginManagerSettings {
    pub(super) fn validate(&self) -> Result<(), Error> {
        if self.namespace.is_empty() || self.target.is_empty() || !self.directory.is_absolute() {
            return Err(Error::new(
                "invalid_settings",
                "Configure Plugin Management",
                "supply a namespace, native target and absolute installation directory",
            ));
        }
        if self.placement.connect_timeout.is_zero() || self.placement.reconnect_delay.is_zero() || self.placement.stop_grace.is_zero() {
            return Err(Error::new("invalid_settings", "Configure Plugin Placement", "supply positive placement durations"));
        }
        if self.command_capacity.get() > tokio::sync::Semaphore::MAX_PERMITS
            || self.watch_capacity.get() > tokio::sync::Semaphore::MAX_PERMITS
        {
            return Err(Error::new(
                "invalid_settings",
                "Configure Plugin Queues",
                "supply capacities representable by the runtime channels",
            ));
        }
        let boundary = v1::ReadCut {
            positions: vec![v1::Position {
                history: Some(v1::History {
                    store: self.namespace.clone(),
                    epoch: crate::bundle::digest(&[]),
                }),
                sequence: u64::MAX,
            }],
            revisions: vec![],
        };
        for action in [super::actor::CAPACITY_ACTION, super::actor::GAP_ACTION] {
            let response = wire::PluginsWatchResponse {
                resolved_at: Some(boundary.clone()),
                event: Some(wire::plugins_watch_response::Event::ResyncRequired(action.into())),
            };
            if response.encoded_len() > self.response_bytes.get() {
                return Err(Error::new(
                    "invalid_settings",
                    "Configure Plugin Read Responses",
                    "provide capacity for a position and its resynchronization action",
                ));
            }
        }
        Ok(())
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
