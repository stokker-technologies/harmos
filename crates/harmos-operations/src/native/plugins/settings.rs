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

use super::Access;
use crate::execution::operations::{ExecutionContext, Grpc, ProtobufOperation, Resource, ServingError, Visibility, access::GrantAccess};
use harmos_protocol::client::plugins as wire;

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

pub struct List;
impl ProtobufOperation for List {
    type Request = wire::SettingsListRequest;
    type Response = wire::SettingsListResponse;
    type Endpoints = ();
    const ID: &'static str = "harmos.client.v1.plugins.Settings/List";
    const GRPC: Option<Grpc> = Some(Grpc::new("harmos.client.v1.plugins", "Settings", "List"));
    const POLICY: crate::OperationPolicy = super::POLICY;
    const VISIBILITY: Visibility = Visibility::Public;
    const RESOURCES: &'static [Resource] = &[Resource::of::<Access>(), Resource::of::<GrantAccess>()];
    const REPLAY: crate::operations::ReplaySafety = crate::operations::ReplaySafety::Idempotent;
    fn grant(_: &Self::Request) -> Result<Option<crate::plugin::Grant>, crate::MessageError> {
        Ok(Some(crate::plugin::Grant::of(Self::ID, "plugins")))
    }
    async fn run(context: ExecutionContext, request: Self::Request, _: &()) -> Result<Self::Response, ServingError> {
        super::admit(&context, Self::ID)?;
        let access = context.resource::<Access>().map_err(|_| ServingError::Catalog)?;
        access.0.settings_list(request).await.map_err(super::failure)
    }
}
pub struct Set;
impl ProtobufOperation for Set {
    type Request = wire::SettingsSetRequest;
    type Response = wire::SettingsSetResponse;
    type Endpoints = ();
    const ID: &'static str = "harmos.client.v1.plugins.Settings/Set";
    const GRPC: Option<Grpc> = Some(Grpc::new("harmos.client.v1.plugins", "Settings", "Set"));
    const POLICY: crate::OperationPolicy = super::POLICY;
    const VISIBILITY: Visibility = Visibility::Public;
    const RESOURCES: &'static [Resource] = &[Resource::of::<Access>(), Resource::of::<GrantAccess>()];
    const REPLAY: crate::operations::ReplaySafety = crate::operations::ReplaySafety::Idempotent;
    fn grant(_: &Self::Request) -> Result<Option<crate::plugin::Grant>, crate::MessageError> {
        Ok(Some(crate::plugin::Grant::of(Self::ID, "plugins")))
    }
    async fn run(context: ExecutionContext, request: Self::Request, _: &()) -> Result<Self::Response, ServingError> {
        super::admit(&context, Self::ID)?;
        let access = context.resource::<Access>().map_err(|_| ServingError::Catalog)?;
        access.0.settings_set(request).await.map_err(super::failure)
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
