/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

use harmos_protocol::host::StatusWatchRequest;
use harmos_protocol::host::StatusWatchResponse;

// Standard Library Imports

// Third Party Imports
use futures_util::StreamExt;

// Local Imports
use super::StatusAccess;
use crate::execution::operations::{ExecutionContext, Grpc, ProtobufOperation, ProtobufOutput, Resource, ServingError, Visibility};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

pub struct Watch;

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl ProtobufOperation for Watch {
    type Request = StatusWatchRequest;
    type Response = StatusWatchResponse;
    type Endpoints = ProtobufOutput<StatusWatchResponse>;
    const ID: &'static str = "harmos.host.v1.Status/Watch";
    const GRPC: Option<Grpc> = Some(Grpc::new("harmos.host.v1", "Status", "Watch"));
    const POLICY: crate::OperationPolicy = super::POLICY;
    const VISIBILITY: Visibility = Visibility::Public;
    const RESOURCES: &'static [Resource] = &[Resource::of::<StatusAccess>()];
    async fn run(context: ExecutionContext, request: Self::Request, output: &Self::Endpoints) -> Result<Self::Response, ServingError> {
        let access = context.resource::<StatusAccess>().map_err(|_| ServingError::Catalog)?;
        let access = access.0.get().ok_or(ServingError::Uninitialized)?;
        let mut events = access.watch(request).await?;
        let mut last = Self::Response::default();
        while let Some(event) = events.next().await {
            last = event?;
            output.send(last.clone()).await.map_err(ServingError::Stream)?;
        }
        Ok(last)
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
