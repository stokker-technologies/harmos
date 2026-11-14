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
use harmos_operations::{Failure, operations::FailureKind};
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
        let name = context.resource::<CallingPlugin>().ok().and_then(|caller| caller.name.clone());
        #[cfg(feature = "plugin-guest")]
        let name = name.or_else(|| context.resource::<crate::guest::PluginScope>().ok().map(|scope| scope.owner().to_owned()));
        let name = name.ok_or_else(|| {
            ServingError::Handler(Failure::new(
                FailureKind::Unauthorized,
                "List Plugin Settings: the calling plugin has no management entry",
            ))
        })?;
        let access = context.resource::<Access>().map_err(|_| ServingError::Catalog)?;
        let result = access
            .0
            .settings_list(plugins::SettingsListRequest { name, at: request.at })
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
