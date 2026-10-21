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
use std::collections::{BTreeMap, BTreeSet};
// Third Party Imports

// Local Imports
use super::{Error, PluginDefinition, PluginGraph};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct ReloadImpact {
    stopping: Vec<PluginDefinition>,
    starting: Vec<PluginDefinition>,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl PluginGraph {
    pub(super) fn reload(&self, next: Self, changed: impl IntoIterator<Item = String>) -> Result<ReloadImpact, Error> {
        let before = by_id(self);
        let after = by_id(&next);
        let mut affected = BTreeSet::new();

        for id in changed {
            if !before.contains_key(id.as_str()) && !after.contains_key(id.as_str()) {
                return Err(Error::Unknown { id });
            }
            affected.insert(id);
        }

        for id in before.keys().chain(after.keys()) {
            if before.get(id) != after.get(id) {
                affected.insert((*id).to_owned());
            }
        }

        loop {
            let previous = affected.len();
            for graph in [self, &next] {
                graph.topology.extend_dependents(&mut affected);
            }
            if affected.len() == previous {
                break;
            }
        }

        let stopping = self.ordered().iter().rev().filter(|artifact| affected.contains(&artifact.id)).cloned().collect();
        let starting = next.ordered().iter().filter(|artifact| affected.contains(&artifact.id)).cloned().collect();

        Ok(ReloadImpact { stopping, starting })
    }
}

impl ReloadImpact {
    pub(super) fn select(&mut self, selected: &BTreeSet<String>) {
        self.stopping.retain(|definition| selected.contains(&definition.id));
        self.starting.retain(|definition| selected.contains(&definition.id));
    }

    pub fn stopping(&self) -> &[PluginDefinition] {
        &self.stopping
    }

    pub fn starting(&self) -> &[PluginDefinition] {
        &self.starting
    }
}

/* === Public Functions === */

/* === Private Functions === */

fn by_id(plan: &PluginGraph) -> BTreeMap<&str, &PluginDefinition> {
    plan.ordered().iter().map(|artifact| (artifact.id.as_str(), artifact)).collect()
}

/* === Tests === */
