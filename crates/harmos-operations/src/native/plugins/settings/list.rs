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

pub struct List;
impl ProtobufOperation for List {
    type Request = wire::SettingsListRequest;
    type Response = wire::SettingsListResponse;
    type Endpoints = ();
    const ID: &'static str = "harmos.host.v1.plugins.Settings/List";
    const GRPC: Option<Grpc> = Some(Grpc::new("harmos.host.v1.plugins", "Settings", "List"));
    const POLICY: crate::OperationPolicy = POLICY;
    const VISIBILITY: Visibility = Visibility::Public;
    const RESOURCES: &'static [Resource] = &[Resource::of::<Access>(), Resource::of::<GrantAccess>()];
    const REPLAY: crate::operations::ReplaySafety = crate::operations::ReplaySafety::Idempotent;
    fn grant(_: &Self::Request) -> Result<Option<crate::plugin::Grant>, crate::MessageError> {
        Ok(None)
    }
    async fn run(context: ExecutionContext, mut request: Self::Request, _: &()) -> Result<Self::Response, ServingError> {
        use crate::execution::operations::access::CallingPlugin;
        if let Ok(caller) = context.resource::<CallingPlugin>() {
            let name = caller.name.as_ref().ok_or_else(|| {
                ServingError::Handler(crate::Failure::new(
                    crate::operations::FailureKind::Unauthorized,
                    "List Plugin Settings: the calling plugin has no management entry",
                ))
            })?;
            if request.name.as_ref().is_some_and(|selected| selected != name) {
                return Err(ServingError::Handler(crate::Failure::new(
                    crate::operations::FailureKind::Unauthorized,
                    "List Plugin Settings: select only the calling plugin",
                )));
            }
            request.name = Some(name.clone());
        } else {
            admit(&context, Self::ID)?;
            if request.name.as_ref().is_none_or(|name| name.is_empty()) {
                return Err(ServingError::Handler(crate::Failure::new(
                    crate::operations::FailureKind::Permanent,
                    "List Plugin Settings: select a plugin entry",
                )));
            }
        }
        let access = context.resource::<Access>().map_err(|_| ServingError::Catalog)?;
        access.0.settings_list(request).await.map_err(failure)
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
