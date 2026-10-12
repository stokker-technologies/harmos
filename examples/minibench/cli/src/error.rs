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
use minibench_app::error::LaunchError;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */
pub type Result<T> = std::result::Result<T, Error>;

/* === Structs === */

/* === Enums === */
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Config(#[from] minibench_app::error::ConfigError),
    #[error(transparent)]
    Runtime(#[from] harmos::Error),
    #[error(transparent)]
    Launch(#[from] LaunchError),
    #[error(transparent)]
    Probe(#[from] minibench_app::error::ProbeError),
    #[error(transparent)]
    BeneathWindow(#[from] harmos::BeneathWindow),
    #[error("the probe sidecar did not publish its reading within five seconds")]
    PublicationTimeout,
}

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
