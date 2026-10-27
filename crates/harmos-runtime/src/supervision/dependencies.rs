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
use std::borrow::Borrow;
use std::collections::{BTreeMap, BTreeSet};

// Third Party Imports
use petgraph::algo::toposort;
use petgraph::graph::DiGraph;

// Local Imports

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Graph<K> {
    dependencies: BTreeMap<K, BTreeSet<K>>,
    dependents: BTreeMap<K, BTreeSet<K>>,
    ordered: Vec<K>,
}

pub(crate) struct Retirement<K> {
    dependencies: BTreeMap<K, BTreeSet<K>>,
    remaining: BTreeMap<K, usize>,
    ready: BTreeSet<K>,
    running: BTreeSet<K>,
}

/* === Enums === */

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Invalid<K> {
    Missing { owner: K, dependency: K },
    Cycle { owner: K },
}

/* === Traits === */

/* === Implementations === */

impl<K: Clone + Ord> Graph<K> {
    pub(crate) fn new(dependencies: BTreeMap<K, BTreeSet<K>>) -> Result<Self, Invalid<K>> {
        let mut graph = DiGraph::<K, ()>::new();
        let nodes: BTreeMap<_, _> = dependencies.keys().map(|id| (id.clone(), graph.add_node(id.clone()))).collect();
        let mut dependents = BTreeMap::<K, BTreeSet<K>>::new();
        for (owner, prerequisites) in &dependencies {
            for dependency in prerequisites {
                let Some(provider) = nodes.get(dependency) else {
                    return Err(Invalid::Missing {
                        owner: owner.clone(),
                        dependency: dependency.clone(),
                    });
                };
                graph.add_edge(*provider, nodes[owner], ());
                dependents.entry(dependency.clone()).or_default().insert(owner.clone());
            }
        }
        // Sorted Node And Edge Insertion Makes Independent Ordering Reproducible.
        let ordered = toposort(&graph, None)
            .map_err(|cycle| Invalid::Cycle { owner: graph[cycle.node_id()].clone() })?
            .into_iter()
            .map(|node| graph[node].clone())
            .collect();
        Ok(Self { dependencies, dependents, ordered })
    }

    pub(crate) fn retirement(&self, selected: BTreeSet<K>) -> Retirement<K> {
        let mut dependencies = BTreeMap::new();
        let mut remaining: BTreeMap<_, usize> = selected.iter().cloned().map(|id| (id, 0)).collect();
        for owner in &selected {
            let providers: BTreeSet<_> = self
                .dependencies
                .get(owner)
                .expect("retirement selects registered graph owners")
                .intersection(&selected)
                .cloned()
                .collect();
            for provider in &providers {
                *remaining.get_mut(provider).unwrap() += 1;
            }
            dependencies.insert(owner.clone(), providers);
        }
        let ready = remaining.iter().filter(|(_, count)| **count == 0).map(|(owner, _)| owner.clone()).collect();
        Retirement {
            dependencies,
            remaining,
            ready,
            running: BTreeSet::new(),
        }
    }

    pub(crate) fn ordered(&self) -> &[K] {
        &self.ordered
    }

    pub(crate) fn dependencies<Q: Ord + ?Sized>(&self, owner: &Q) -> Option<&BTreeSet<K>>
    where
        K: Borrow<Q>,
    {
        self.dependencies.get(owner)
    }

    pub(crate) fn extend_dependents(&self, affected: &mut BTreeSet<K>) {
        let mut pending: Vec<_> = affected.iter().cloned().collect();
        while let Some(owner) = pending.pop() {
            if let Some(dependents) = self.dependents.get(&owner) {
                for dependent in dependents {
                    if affected.insert(dependent.clone()) {
                        pending.push(dependent.clone());
                    }
                }
            }
        }
    }
}

impl<K: Clone + Ord> Retirement<K> {
    pub(crate) fn take_ready(&mut self) -> BTreeSet<K> {
        let ready = std::mem::take(&mut self.ready);
        self.running.extend(ready.iter().cloned());
        ready
    }

    pub(crate) fn complete(&mut self, owner: &K, settled: bool) {
        assert!(self.running.remove(owner), "only dispatched cleanup can complete");
        self.remaining.remove(owner);
        if !settled {
            return;
        }
        for provider in &self.dependencies[owner] {
            let remaining = self.remaining.get_mut(provider).expect("a provider cannot retire before its dependents");
            *remaining -= 1;
            if *remaining == 0 {
                self.ready.insert(provider.clone());
            }
        }
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
