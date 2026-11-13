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

pub enum PluginBackend<G, E> {
    Guest(G),
    Embedded(E),
}

/* === Traits === */

/* === Implementations === */

impl<G, E> PluginBackend<G, E> {
    pub fn kind(&self) -> PluginKind {
        match self {
            Self::Guest(_) => PluginKind::Guest,
            Self::Embedded(_) => PluginKind::Embedded,
        }
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
