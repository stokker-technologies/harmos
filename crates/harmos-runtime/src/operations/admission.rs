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
use super::lane;
use crate::{Error, Result};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

#[derive(Default)]
pub(super) struct Lanes {
    edges: Mutex<BTreeMap<(usize, usize), usize>>,
}

#[derive(Clone)]
pub(super) struct Admission {
    held: Arc<[lane::Admission]>,
    _claim: Arc<Claim>,
}

struct Claim {
    lanes: Weak<Lanes>,
    edges: Vec<(usize, usize)>,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl Lanes {
    pub(super) fn admit(self: &Arc<Self>, ancestors: &[lane::Admission], lane: &lane::Admission) -> Result<Admission> {
        let dependencies =
            if lane.bounded() { ancestors.iter().map(|parent| (parent.key(), lane.key())).collect::<Vec<_>>() } else { Vec::new() };
        let mut edges = self.edges.lock().expect("operation lane graph is healthy");

        // Validate the whole claim before mutating counts. Checking the reverse
        // path also catches two independent roots calling each other's lane.
        for &(parent, child) in &dependencies {
            if reaches(&edges, child, parent) {
                let parent = ancestors.iter().find(|lane| lane.key() == parent).expect("held parent lane").name().to_owned();
                let child = lane.name().to_owned();
                return Err(Error::OperationLaneCycle { parent, child });
            }
        }
        for &edge in &dependencies {
            *edges.entry(edge).or_default() += 1;
        }
        drop(edges);

        let mut held = ancestors.to_vec();
        if lane.bounded() {
            held.push(lane.clone());
        }
        Ok(Admission {
            held: held.into(),
            _claim: Arc::new(Claim {
                lanes: Arc::downgrade(self),
                edges: dependencies,
            }),
        })
    }
}

impl Admission {
    pub(super) fn held(&self) -> &[lane::Admission] {
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

fn reaches(edges: &BTreeMap<(usize, usize), usize>, from: usize, to: usize) -> bool {
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
