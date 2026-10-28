/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

use std::collections::BTreeMap;
use std::sync::Arc;

use tokio::sync::Semaphore;

// The lane resident controls run in: unbounded, so a control that runs until
// cancelled never holds a slot another call is waiting for.
pub(crate) const RESIDENT: &str = "harmos.resident";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Capacity {
    width: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LaneSettings {
    pub capacity: Capacity,
}

#[derive(Clone, Debug, Default)]
pub struct LaneSet {
    entries: Vec<(&'static str, Capacity)>,
}

#[derive(Clone)]
pub(crate) struct Admission {
    name: &'static str,
    width: usize,
    semaphore: Arc<Semaphore>,
}

pub(crate) struct Lanes {
    lanes: BTreeMap<&'static str, Admission>,
}

pub trait Lane: Copy + Eq + Send + Sync + 'static {
    fn catalog() -> LaneSet;
    fn default_lane() -> Self;
    fn name(&self) -> &'static str;
    fn capacity(&self) -> Capacity;
}

impl Capacity {
    pub const fn serial() -> Self {
        Self { width: 1 }
    }

    pub const fn concurrent(width: usize) -> Self {
        Self { width }
    }

    pub const fn unbounded() -> Self {
        Self { width: usize::MAX }
    }

    pub(crate) const fn width(self) -> usize {
        self.width
    }
}

impl LaneSet {
    pub fn declared(entries: impl IntoIterator<Item = (&'static str, Capacity)>) -> Self {
        Self { entries: entries.into_iter().collect() }
    }

    pub(crate) fn entries(self) -> Vec<(&'static str, Capacity)> {
        self.entries
    }

    pub(crate) fn contains(&self, lane: &str) -> bool {
        self.entries.iter().any(|(name, _)| *name == lane)
    }

    pub(crate) fn configure(&mut self, lane: &str, capacity: Capacity) -> bool {
        let Some((_, configured)) = self.entries.iter_mut().find(|(name, _)| *name == lane) else {
            return false;
        };
        *configured = capacity;
        true
    }
}

impl Admission {
    pub(crate) fn name(&self) -> &'static str {
        self.name
    }

    pub(crate) fn bounded(&self) -> bool {
        self.width != usize::MAX
    }

    pub(crate) fn semaphore(&self) -> Arc<Semaphore> {
        Arc::clone(&self.semaphore)
    }
}

impl Lanes {
    pub(crate) fn freeze(declared: LaneSet) -> Result<Self, String> {
        let mut lanes = BTreeMap::from([(
            RESIDENT,
            Admission {
                name: RESIDENT,
                width: usize::MAX,
                semaphore: Arc::new(Semaphore::new(Semaphore::MAX_PERMITS)),
            },
        )]);

        for (name, capacity) in declared.entries() {
            if name.is_empty() {
                return Err("a lane name must not be empty".to_owned());
            }
            if capacity.width() == 0 {
                return Err(format!("lane `{name}` must admit at least one job"));
            }
            let admission = Admission {
                name,
                width: capacity.width(),
                semaphore: Arc::new(Semaphore::new(capacity.width().min(Semaphore::MAX_PERMITS))),
            };
            if lanes.insert(name, admission).is_some() {
                return Err(format!("lane `{name}` is declared more than once"));
            }
        }

        Ok(Self { lanes })
    }

    pub(crate) fn get(&self, name: &str) -> Option<Admission> {
        self.lanes.get(name).cloned()
    }
}
