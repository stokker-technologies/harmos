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
use std::sync::{Arc, Mutex, Weak};
// Third Party Imports

// Local Imports
use crate::{Error, Result};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */
#[derive(Default)]
pub(super) struct Lanes {
    edges: Mutex<BTreeMap<(&'static str, &'static str), usize>>,
}

#[derive(Clone)]
pub(super) struct Admission {
    held: Arc<[&'static str]>,
    _claim: Arc<Claim>,
}

struct Claim {
    lanes: Weak<Lanes>,
    edges: Vec<(&'static str, &'static str)>,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */
impl Lanes {
    pub(super) fn admit(self: &Arc<Self>, ancestors: &[&'static str], lane: &'static str, bounded: bool) -> Result<Admission> {
        let dependencies = if bounded { ancestors.iter().map(|&parent| (parent, lane)).collect::<Vec<_>>() } else { Vec::new() };
        let mut edges = self.edges.lock().expect("operation lane graph is healthy");

        // Validate the whole claim before mutating counts. Checking the reverse
        // path also catches two independent roots calling each other's lane.
        for &(parent, child) in &dependencies {
            if reaches(&edges, child, parent) {
                return Err(Error::OperationLaneCycle { parent, child });
            }
        }
        for &edge in &dependencies {
            *edges.entry(edge).or_default() += 1;
        }
        drop(edges);

        let mut held = ancestors.to_vec();
        if bounded {
            held.push(lane);
        }
        Ok(Admission { held: held.into(), _claim: Arc::new(Claim { lanes: Arc::downgrade(self), edges: dependencies }) })
    }
}

impl Admission {
    pub(super) fn held(&self) -> &[&'static str] {
        &self.held
    }
}

impl Drop for Claim {
    fn drop(&mut self) {
        let Some(lanes) = self.lanes.upgrade() else {
            return;
        };
        let mut edges = lanes.edges.lock().expect("operation lane graph is healthy");
        for edge in &self.edges {
            let count = edges.get_mut(edge).expect("admitted dependency is retained");
            *count -= 1;
            if *count == 0 {
                edges.remove(edge);
            }
        }
    }
}

/* === Public Functions === */

/* === Private Functions === */
fn reaches(edges: &BTreeMap<(&'static str, &'static str), usize>, from: &'static str, to: &'static str) -> bool {
    let mut pending = vec![from];
    let mut visited = BTreeSet::new();
    while let Some(lane) = pending.pop() {
        if lane == to {
            return true;
        }
        if visited.insert(lane) {
            pending.extend(edges.keys().filter_map(|&(parent, child)| (parent == lane).then_some(child)));
        }
    }
    false
}

/* === Tests === */
