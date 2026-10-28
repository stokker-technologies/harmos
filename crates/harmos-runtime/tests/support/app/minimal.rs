/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports

// Third Party Imports

// Local Imports
use harmos_runtime::{Application, state::Never};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

pub struct App;

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl harmos_runtime::Lifecycle for App {
    type Config = ();
    type Error = std::convert::Infallible;
    type Context = harmos_runtime::Startup<Self>;

    async fn prepare(_: (), _: &mut harmos_runtime::Setup<Self>) -> std::result::Result<Self, Self::Error> {
        Ok(Self)
    }

    async fn initialize(&mut self, _: &Self::Context) -> std::result::Result<(), Self::Error> {
        Ok(())
    }

    async fn stop(&mut self, _: &Self::Context) -> std::result::Result<(), Self::Error> {
        Ok(())
    }

    async fn cleanup(&mut self) -> std::result::Result<(), Self::Error> {
        Ok(())
    }
}

impl Application for App {
    type Lanes = super::Lanes;
    type State = ();
    type Transaction = Never;
    type Record = ();
    type Streams = ();
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
