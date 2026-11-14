/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Plugin Settings Operations

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

use super::super::{Access, POLICY, admit, failure};
use crate::execution::operations::{ExecutionContext, Grpc, ProtobufOperation, Resource, ServingError, Visibility, access::GrantAccess};
use harmos_protocol::host::plugins as wire;

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

pub struct Set;
impl ProtobufOperation for Set {
    type Request = wire::SettingsSetRequest;
    type Response = wire::SettingsSetResponse;
    type Endpoints = ();
    const ID: &'static str = "harmos.host.v1.plugins.Settings/Set";
    const GRPC: Option<Grpc> = Some(Grpc::new("harmos.host.v1.plugins", "Settings", "Set"));
    const POLICY: crate::OperationPolicy = POLICY;
    const VISIBILITY: Visibility = Visibility::Public;
    const RESOURCES: &'static [Resource] = &[Resource::of::<Access>(), Resource::of::<GrantAccess>()];
    const REPLAY: crate::operations::ReplaySafety = crate::operations::ReplaySafety::Idempotent;
    fn grant(_: &Self::Request) -> Result<Option<crate::plugin::Grant>, crate::MessageError> {
        Ok(Some(crate::plugin::Grant::of(Self::ID, "plugins")))
    }
    async fn run(context: ExecutionContext, request: Self::Request, _: &()) -> Result<Self::Response, ServingError> {
        admit(&context, Self::ID)?;
        let access = context.resource::<Access>().map_err(|_| ServingError::Catalog)?;
        access.0.settings_set(request).await.map_err(failure)
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
