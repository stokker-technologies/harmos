/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Lifecycle Preparation And Role-Specific Configuration

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::sync::Arc;

// Third Party Imports

// Local Imports
use crate::Lifecycle;
use crate::lifecycle::Context;
use crate::operations::{OperationCatalog, StartupError};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

pub struct Setup<L: Lifecycle> {
    configuration: <L::Context as Context>::Configuration,
    replacing: bool,
}

/* === Enums === */

/* === Traits === */

#[doc(hidden)]
pub trait SetupConfiguration: Send + 'static {
    fn resource<T: Send + Sync + 'static>(&mut self, value: Arc<T>, replace: bool);
    fn validate(&mut self) -> Result<(), StartupError>;
}

#[doc(hidden)]
pub trait ApplicationConfiguration: SetupConfiguration {
    type Lane;
    type LaneSettings;

    fn lane(&mut self, lane: Self::Lane, settings: Self::LaneSettings);
    fn operations<C: OperationCatalog>(&mut self, lane: Self::Lane);
}

/* === Implementations === */

impl<L: Lifecycle> Setup<L> {
    #[doc(hidden)]
    pub fn new(configuration: <L::Context as Context>::Configuration) -> Self {
        Self { configuration, replacing: false }
    }

    pub fn resource<T: Send + Sync + 'static>(&mut self, value: impl Into<Arc<T>>) -> &mut Self {
        self.configuration.resource(value.into(), self.replacing);
        self
    }

    pub fn settings(&mut self) -> &mut <L::Context as Context>::Configuration {
        &mut self.configuration
    }

    #[doc(hidden)]
    pub fn overrides(&mut self, update: impl FnOnce(&mut Self)) {
        self.replacing = true;
        update(self);
        self.replacing = false;
    }

    #[doc(hidden)]
    pub fn validate(&mut self) -> Result<(), StartupError> {
        self.configuration.validate()
    }

    #[doc(hidden)]
    pub fn into_configuration(self) -> <L::Context as Context>::Configuration {
        self.configuration
    }
}

impl<L: Lifecycle> Setup<L>
where
    <L::Context as Context>::Configuration: ApplicationConfiguration,
{
    pub fn lane(
        &mut self, lane: <<L::Context as Context>::Configuration as ApplicationConfiguration>::Lane,
        settings: <<L::Context as Context>::Configuration as ApplicationConfiguration>::LaneSettings,
    ) -> &mut Self {
        self.configuration.lane(lane, settings);
        self
    }

    pub fn operations<C: OperationCatalog>(
        &mut self, lane: <<L::Context as Context>::Configuration as ApplicationConfiguration>::Lane,
    ) -> &mut Self {
        self.configuration.operations::<C>(lane);
        self
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
