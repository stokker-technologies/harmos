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
use super::{Access, admit};
use crate::execution::operations::{ExecutionContext, Grpc, ProtobufOperation, ProtobufOutput, Resource, ServingError, Visibility};
use harmos_protocol::client as wire;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

pub struct Watch;

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl ProtobufOperation for Watch {
    type Request = wire::PluginsWatchRequest;
    type Response = wire::PluginsWatchResponse;
    type Endpoints = ProtobufOutput<Self::Response>;
    const ID: &'static str = "harmos.client.v1.Plugins/Watch";
    const GRPC: Option<Grpc> = Some(Grpc::new("harmos.client.v1", "Plugins", "Watch"));
    const POLICY: crate::OperationPolicy = super::WATCH_POLICY;
    const VISIBILITY: Visibility = Visibility::Public;
    const RESOURCES: &'static [Resource] = &[Resource::of::<Access>(), Resource::of::<crate::execution::operations::access::GrantAccess>()];
    fn grant(_request: &Self::Request) -> Result<Option<crate::plugin::Grant>, harmos_domain::MessageError> {
        Ok(Some(crate::plugin::Grant::of(Self::ID, "plugins")))
    }
    async fn run(context: ExecutionContext, request: Self::Request, output: &Self::Endpoints) -> Result<Self::Response, ServingError> {
        admit(&context, Self::ID)?;
        let access = context.resource::<Access>().map_err(|_| ServingError::Catalog)?;
        let mut changes = access.0.watch(request).await.map_err(super::failure)?;
        let mut last = Self::Response::default();
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
