/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Plugin Resource Management

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports

// Third Party Imports

// Local Imports
use super::Access;
use crate::execution::operations::{ExecutionContext, Grpc, ProtobufOperation, ProtobufOutput, Resource, ServingError, Visibility};
use crate::host::declarations::Events;
use harmos_protocol::host;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

pub struct Watch;

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl ProtobufOperation for Watch {
    type Request = host::PluginsWatchRequest;
    type Response = host::PluginsWatchResponse;
    type Endpoints = ProtobufOutput<Self::Response>;
    const ID: &'static str = "harmos.host.v1.Plugins/Watch";
    const GRPC: Option<Grpc> = None;
    const POLICY: crate::OperationPolicy = super::WATCH_POLICY;
    const VISIBILITY: Visibility = Visibility::Private;
    const RESOURCES: &'static [Resource] = &[Resource::of::<Access>()];
    fn grant(_request: &Self::Request) -> Result<Option<crate::plugin::Grant>, harmos_domain::MessageError> {
        Ok(Some(crate::plugin::Grant::of(Self::ID, "plugins")))
    }
    async fn run(context: ExecutionContext, request: Self::Request, output: &Self::Endpoints) -> Result<Self::Response, ServingError> {
        Self::run_with(context, request, output).await
    }
}

impl Watch {
    pub(crate) async fn run_with(
        context: ExecutionContext, request: host::PluginsWatchRequest, output: &impl Events<host::PluginsWatchResponse>,
    ) -> Result<host::PluginsWatchResponse, ServingError> {
        let access = context.resource::<Access>().map_err(|_| ServingError::Catalog)?;
        let mut changes = access.0.watch(request).await.map_err(super::failure)?;
        let mut last = host::PluginsWatchResponse::default();
        loop {
            tokio::select! {
                biased;
                _ = context.cancellation.cancelled() => return Ok(last),
                event = changes.recv() => match event {
                    Some(event) => {
                        last = event;
                        output.send(last.clone()).await.map_err(ServingError::Stream)?;
                    },
                    None => return Ok(last),
                }
            }
        }
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
