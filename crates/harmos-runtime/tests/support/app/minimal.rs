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

    async fn prepare(_: (), setup: &mut harmos_runtime::Setup<Self>) -> std::result::Result<Self, Self::Error> {
        setup.settings().lanes(
            harmos_runtime::operations::LaneSet::new([crate::support::SERIAL, crate::support::CONCURRENT, crate::support::UNBOUNDED])
                .unwrap(),
        );
        Ok(Self)
    }
}

impl Application for App {
    type State = ();
    type Transaction = Never;
    type Record = ();
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
