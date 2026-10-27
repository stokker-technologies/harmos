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
#[harmos_runtime::operation(id = "scale_privately",
    policy = GAUGE_POLICY,
    response = Scale,
)]
#[derive(Clone, PartialEq)]
pub struct ScalePrivatelyDeclaration {
    pub value: i64,
}

#[harmos_runtime::operation(id = "scale_publicly",
    policy = GAUGE_POLICY,
    response = Scale,
)]
#[derive(Clone, PartialEq)]
pub struct ScalePubliclyDeclaration {}

pub(super) struct ScalingPrivately {
    resources: Arc<GaugeResources>,
}

impl harmos_runtime::operations::Handler for ScalingPrivately {
    type Request = <ScalePrivatelyDeclaration as harmos_runtime::OperationDeclaration>::Request;
    type Response = <ScalePrivatelyDeclaration as harmos_runtime::OperationDeclaration>::Response;

    type Error = Refusal;
    type StreamEndpoints = ();
    async fn initialize(context: harmos_runtime::OperationContext) -> Result<Self, Refusal> {
        let resources = context.resource::<GaugeResources>().expect("fixture dependency");
        Ok(Self { resources })
    }
    async fn run(
        &mut self, _: harmos_runtime::OperationContext, request: &mut ScalePrivatelyDeclaration, _endpoints: &mut Self::StreamEndpoints,
    ) -> Result<harmos_runtime::operations::Response<Scale>, Refusal> {
        let result: ::core::result::Result<Scale, Self::Error> = async move {
            Ok(Scale {
                value: request.value * self.resources.factor,
            })
        }
        .await;
        result.map(::core::convert::Into::into)
    }
}
pub(super) struct ScalingPublicly {
    resources: Arc<GaugeResources>,
}

impl harmos_runtime::operations::Handler for ScalingPublicly {
    type Request = <ScalePubliclyDeclaration as harmos_runtime::OperationDeclaration>::Request;
    type Response = <ScalePubliclyDeclaration as harmos_runtime::OperationDeclaration>::Response;

    type Error = Refusal;
    type StreamEndpoints = ();
    async fn initialize(context: harmos_runtime::OperationContext) -> Result<Self, Refusal> {
        let resources = context.resource::<GaugeResources>().expect("fixture dependency");
        Ok(Self { resources })
    }
    async fn run(
        &mut self, _: harmos_runtime::OperationContext, _request: &mut ScalePubliclyDeclaration, _endpoints: &mut Self::StreamEndpoints,
    ) -> Result<harmos_runtime::operations::Response<Scale>, Refusal> {
        let result: ::core::result::Result<Scale, Self::Error> = async move { Ok(Scale { value: self.resources.factor }) }.await;
        result.map(::core::convert::Into::into)
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
