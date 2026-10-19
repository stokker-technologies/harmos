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

pub enum PluginBackend<G, S, E> {
    Guest(G),
    Sidecar(S),
    Embedded(E),
}

/* === Traits === */

/* === Implementations === */

impl<G, S, E> PluginBackend<G, S, E> {
    pub fn kind(&self) -> PluginKind {
        match self {
            Self::Guest(_) => PluginKind::Guest,
            Self::Sidecar(_) => PluginKind::Sidecar,
            Self::Embedded(_) => PluginKind::Embedded,
        }
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
