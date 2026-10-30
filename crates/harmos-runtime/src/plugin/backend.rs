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
use super::PluginKind;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

pub enum PluginBackend<S, E> {
    Sidecar(S),
    Embedded(E),
}

/* === Traits === */

/* === Implementations === */

impl<S, E> PluginBackend<S, E> {
    pub fn kind(&self) -> PluginKind {
        match self {
            Self::Sidecar(_) => PluginKind::Sidecar,
            Self::Embedded(_) => PluginKind::Embedded,
        }
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
