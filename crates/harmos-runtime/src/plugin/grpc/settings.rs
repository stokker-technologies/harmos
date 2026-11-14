/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Calling Plugin Settings

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

use super::CallingPlugin;
use harmos_operations::execution::operations::{ExecutionContext, ProtobufOperation, Resource, ServingError, Visibility};
use harmos_operations::plugins::Access;
use harmos_protocol::{client::plugins, plugin::host};

// Standard Library Imports

// Third Party Imports

// Local Imports

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

pub(crate) struct List;

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl ProtobufOperation for List {
    type Request = host::SettingsListRequest;
    type Response = host::SettingsListResponse;
    type Endpoints = ();
    const ID: &'static str = "harmos.plugin.host.v1.Settings/List";
    const POLICY: harmos_operations::OperationPolicy = harmos_operations::host::POLICY;
    const VISIBILITY: Visibility = Visibility::Public;
    const RESOURCES: &'static [Resource] = &[Resource::of::<CallingPlugin>(), Resource::of::<Access>()];
    async fn run(context: ExecutionContext, request: Self::Request, _: &()) -> Result<Self::Response, ServingError> {
        let caller = context.resource::<CallingPlugin>().map_err(|_| ServingError::Catalog)?;
        let access = context.resource::<Access>().map_err(|_| ServingError::Catalog)?;
        let result = access
            .0
            .settings_list(plugins::SettingsListRequest {
                name: caller.name.clone(),
                at: request.at,
            })
            .await
            .map_err(|error| ServingError::Handler((*error.0).into()))?;
        Ok(host::SettingsListResponse {
            value: result.value,
            revision: result.revision,
            resolved_at: result.resolved_at,
        })
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
