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
use crate::execution::operations::{ExecutionContext, Grpc, ProtobufOperation, Resource, ServingError, Visibility};
use harmos_protocol::host as wire;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

pub struct Set;

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl ProtobufOperation for Set {
    type Request = wire::PluginsSetRequest;
    type Response = wire::PluginsSetResponse;
    type Endpoints = ();
    const ID: &'static str = "harmos.host.v1.Plugins/Set";
    const GRPC: Option<Grpc> = Some(Grpc::new("harmos.host.v1", "Plugins", "Set"));
    const POLICY: crate::OperationPolicy = super::POLICY;
    const VISIBILITY: Visibility = Visibility::Public;
    const REPLAY: crate::operations::ReplaySafety = crate::operations::ReplaySafety::Idempotent;
    const RESOURCES: &'static [Resource] = &[Resource::of::<Access>(), Resource::of::<crate::execution::operations::access::GrantAccess>()];
    fn grant(_request: &Self::Request) -> Result<Option<crate::plugin::Grant>, harmos_domain::MessageError> {
        Ok(Some(crate::plugin::Grant::of(Self::ID, "plugins")))
    }
    async fn run(context: ExecutionContext, request: Self::Request, _output: &()) -> Result<Self::Response, ServingError> {
        admit(&context, Self::ID)?;
        let access = context.resource::<Access>().map_err(|_| ServingError::Catalog)?;
        access.0.set(request).await.map_err(super::failure)
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
