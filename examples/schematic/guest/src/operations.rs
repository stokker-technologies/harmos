/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Unary Guest Operations

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports

// Third Party Imports

// Local Imports
use harmos::plugin::{Context, HostOperation, Operation, Registrar, Registration};
use harmos::setting_value::Value;
use harmos::{Error, OperationRequest, Result, Visibility};
use schematic_protocol::Point;
use schematic_protocol::plugin::{
    SchematicProposeRouteRequest, SchematicProposeRouteResponse, SchematicValidateRequest, SchematicValidateResponse,
};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

pub fn catalog() -> impl Registration {
    move |ctx: &mut Registrar<'_>| {
        ctx.register(schematic_protocol::settings_declarations())?;
        ctx.register(HostOperation::PluginsSettingsList)?;
        ctx.register(Operation::new(SchematicValidateRequest::ID, validate).visibility(Visibility::Public).policy(super::POLICY))?;
        ctx.register(Operation::new(SchematicProposeRouteRequest::ID, route).visibility(Visibility::Public).policy(super::POLICY))?;
        Ok(())
    }
}

/* === Private Functions === */

async fn validate(_: &Context, request: SchematicValidateRequest) -> Result<SchematicValidateResponse> {
    let schematic: schematic_domain::model::Snapshot =
        request.schematic.ok_or_else(|| Error::Invalid("Validate Schematic: Missing Snapshot".into()))?.into();
    Ok(SchematicValidateResponse { diagnostics: schematic.diagnostics() })
}
async fn route(context: &Context, request: SchematicProposeRouteRequest) -> Result<SchematicProposeRouteResponse> {
    let from = request.from.ok_or_else(|| Error::Invalid("Propose Route: Missing From Point".into()))?;
    let to = request.to.ok_or_else(|| Error::Invalid("Propose Route: Missing To Point".into()))?;
    let settings = context.host().plugins.settings.list().await?.value.ok_or_else(|| Error::Invalid("Route: Missing Settings".into()))?;
    let Some(Value::Flag(horizontal_first)) = settings.values.get("horizontal_first").and_then(|value| value.value.as_ref()) else {
        return Err(Error::Invalid("Route: Missing Horizontal First Flag".into()));
    };
    let corner = if *horizontal_first { Point { x: to.x, y: from.y } } else { Point { x: from.x, y: to.y } };
    let mut points = vec![from, corner, to];
    points.dedup();
    Ok(SchematicProposeRouteResponse { points })
}

/* === Tests === */
