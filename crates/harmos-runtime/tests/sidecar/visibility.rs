/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Bind Private And Shared Native Handlers To Their Original Resources.

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports

// Third Party Imports

// Local Imports
use super::*;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

#[harmos_runtime::message]
#[derive(Clone, PartialEq)]
pub struct Scale {
    pub value: i64,
}
#[harmos_runtime::operation(resources = [GaugeResources], id = "scale.privately",
    policy = GAUGE_POLICY,
    response = Scale,
)]
#[derive(Clone, PartialEq)]
pub struct ScalePrivatelyDeclaration {
    pub value: i64,
}

#[harmos_runtime::operation(resources = [GaugeResources], id = "scale.publicly",
    policy = GAUGE_POLICY,
    response = Scale,
)]
#[derive(Clone, PartialEq)]
pub struct ScalePubliclyDeclaration {}

impl harmos_runtime::Operation for ScalePrivatelyDeclaration {
    type Error = Refusal;

    async fn run(self, ctx: harmos_runtime::Context<'_, Self>) -> Result<harmos_runtime::operations::Response<Scale>, Refusal> {
        let resources = ctx.resource::<GaugeResources>();
        let result: ::core::result::Result<Scale, Self::Error> = async move { Ok(Scale { value: self.value * resources.factor }) }.await;
        result.map(::core::convert::Into::into)
    }
}

impl harmos_runtime::Operation for ScalePubliclyDeclaration {
    type Error = Refusal;

    async fn run(self, ctx: harmos_runtime::Context<'_, Self>) -> Result<harmos_runtime::operations::Response<Scale>, Refusal> {
        let resources = ctx.resource::<GaugeResources>();
        let result: ::core::result::Result<Scale, Self::Error> = async move { Ok(Scale { value: resources.factor }) }.await;
        result.map(::core::convert::Into::into)
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
