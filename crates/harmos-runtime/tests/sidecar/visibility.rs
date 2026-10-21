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
#[harmos_runtime::message]
#[harmos_runtime::operation(
    id = "gauge.scale-privately",
    version = 1,
    policy = GAUGE_POLICY,
    response = Scale,
)]
#[derive(Clone, PartialEq)]
pub struct ScalePrivately {
    pub value: i64,
}
#[harmos_runtime::message]
#[harmos_runtime::operation(
    id = "gauge.scale-publicly",
    version = 1,
    policy = GAUGE_POLICY,
    response = Scale,
)]
#[derive(Clone, PartialEq)]
pub struct ScalePublicly {}
pub(super) struct ScalingPrivately {
    resources: Arc<GaugeResources>,
}

impl harmos_runtime::operations::Operation for ScalingPrivately {
    type Declaration = ScalePrivately;
    type Resources = GaugeResources;
    type Error = Refusal;
    type Endpoints = ();
    type Context<'a> = &'a Supervisor;
    async fn initialize<'ctx>(resources: Arc<GaugeResources>, _: Self::Context<'ctx>) -> Result<Self, Refusal> {
        Ok(Self { resources })
    }
    async fn run<'ctx>(
        &mut self, _: Self::Context<'ctx>, request: &mut ScalePrivately, _endpoints: &mut Self::Endpoints,
    ) -> Result<harmos_runtime::operations::Response<Scale>, Refusal> {
        let result: ::core::result::Result<Scale, Self::Error> =
            async move { Ok(Scale { value: request.value * self.resources.factor }) }.await;
        result.map(::core::convert::Into::into)
    }
}
pub(super) struct ScalingPublicly {
    resources: Arc<GaugeResources>,
}

impl harmos_runtime::operations::Operation for ScalingPublicly {
    type Declaration = ScalePublicly;
    type Resources = GaugeResources;
    type Error = Refusal;
    type Endpoints = ();
    type Context<'a> = &'a Supervisor;
    async fn initialize<'ctx>(resources: Arc<GaugeResources>, _: Self::Context<'ctx>) -> Result<Self, Refusal> {
        Ok(Self { resources })
    }
    async fn run<'ctx>(
        &mut self, _: Self::Context<'ctx>, _request: &mut ScalePublicly, _endpoints: &mut Self::Endpoints,
    ) -> Result<harmos_runtime::operations::Response<Scale>, Refusal> {
        let result: ::core::result::Result<Scale, Self::Error> = async move { Ok(Scale { value: self.resources.factor }) }.await;
        result.map(::core::convert::Into::into)
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
