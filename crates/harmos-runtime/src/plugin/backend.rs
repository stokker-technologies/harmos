/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

use super::PluginKind;

pub enum PluginBackend<G, S, E> {
    Guest(G),
    Sidecar(S),
    Embedded(E),
}

impl<G, S, E> PluginBackend<G, S, E> {
    pub fn kind(&self) -> PluginKind {
        match self {
            Self::Guest(_) => PluginKind::Guest,
            Self::Sidecar(_) => PluginKind::Sidecar,
            Self::Embedded(_) => PluginKind::Embedded,
        }
    }
}
