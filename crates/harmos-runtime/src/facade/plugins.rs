/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Typed Plugin Calls

/* === Modules === */

/* === Re-Exports === */

pub use crate::plugin::grpc::facade::{
    BindRequest, Binding, InspectRequest, Inspection, LoadRequest, Plugin, Samples, SettleRequest, StartSamplesRequest, StopSamplesRequest,
};
pub use crate::plugin::grpc::{Managed, Placement, PlacementSettings};

/* === Imports === */

// Standard Library Imports

// Third Party Imports

// Local Imports
use super::Call;
use crate::Result;
use crate::plugin::grpc::facade;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

pub struct Plugins<'a> {
    pub(super) call: &'a Call,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl Plugins<'_> {
    pub async fn load(&self, request: LoadRequest) -> Result<Plugin> {
        self.call.call::<facade::Load>(request).await
    }

    pub async fn bind(&self, request: BindRequest) -> Result<Binding> {
        self.call.call::<facade::Bind>(request).await
    }

    pub async fn start_samples(&self, request: StartSamplesRequest) -> Result<Samples> {
        self.call.call::<facade::StartSamples>(request).await
    }

    pub async fn stop_samples(&self, request: StopSamplesRequest) -> Result<()> {
        self.call.call::<facade::StopSamples>(request).await
    }

    pub async fn settle(&self, request: SettleRequest) -> Result<()> {
        self.call.call::<facade::Settle>(request).await
    }

    pub async fn inspect(&self, request: InspectRequest) -> Result<Inspection> {
        self.call.call::<facade::Inspect>(request).await
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
