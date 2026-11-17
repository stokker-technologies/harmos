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
use crate::execution::operations::{ExecutionContext, Grpc, ProtobufOperation, Resource, ServingError, Visibility};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

pub struct Get;

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl ProtobufOperation for Get {
    type Request = harmos_protocol::host::PluginsGetRequest;
    type Response = harmos_protocol::host::PluginsGetResponse;
    type Endpoints = ();
    const ID: &'static str = "harmos.host.v1.Plugins/Get";
    const GRPC: Option<Grpc> = None;
    const POLICY: crate::OperationPolicy = super::POLICY;
    const VISIBILITY: Visibility = Visibility::Private;
    const REPLAY: crate::operations::ReplaySafety = crate::operations::ReplaySafety::Idempotent;
    const RESOURCES: &'static [Resource] = &[Resource::of::<Access>()];
    fn grant(_request: &Self::Request) -> Result<Option<crate::plugin::Grant>, harmos_domain::MessageError> {
        Ok(Some(crate::plugin::Grant::of(Self::ID, "plugins")))
    }
    async fn run(context: ExecutionContext, request: Self::Request, _output: &()) -> Result<Self::Response, ServingError> {
        let access = context.resource::<Access>().map_err(|_| ServingError::Catalog)?;
        access.0.get(request).await.map_err(super::failure)
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
