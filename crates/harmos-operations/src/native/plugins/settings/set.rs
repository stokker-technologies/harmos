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

// Standard Library Imports

// Third Party Imports

// Local Imports
use super::super::{Access, POLICY, failure};
use crate::execution::operations::{ExecutionContext, Grpc, ProtobufOperation, Resource, ServingError, Visibility};
use crate::{Failure, operations::FailureKind};
use harmos_protocol::host;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

pub struct Set;
impl ProtobufOperation for Set {
    type Request = host::plugins::SettingsSetRequest;
    type Response = host::plugins::SettingsSetResponse;
    type Endpoints = ();
    const ID: &'static str = "harmos.host.v1.plugins.Settings/Set";
    const GRPC: Option<Grpc> = None;
    const POLICY: crate::OperationPolicy = POLICY;
    const VISIBILITY: Visibility = Visibility::Private;
    const RESOURCES: &'static [Resource] = &[Resource::of::<Access>()];
    const REPLAY: crate::operations::ReplaySafety = crate::operations::ReplaySafety::Idempotent;
    fn grant(_: &Self::Request) -> Result<Option<crate::plugin::Grant>, crate::MessageError> {
        Ok(Some(crate::plugin::Grant::of(Self::ID, "plugins")))
    }
    async fn run(context: ExecutionContext, request: Self::Request, _: &()) -> Result<Self::Response, ServingError> {
        let access = context.resource::<Access>().map_err(|_| ServingError::Catalog)?;
        access.0.settings_set(request).await.map_err(|error| {
            if error.0.code.starts_with("setting_") || error.0.code == "value_required" {
                return ServingError::Handler(Failure::new(FailureKind::Invalid, error.to_string()));
            }
            failure(error)
        })
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
