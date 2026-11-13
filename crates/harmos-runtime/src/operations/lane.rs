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
use std::collections::BTreeMap;
use std::sync::Arc;
// Third Party Imports
use tokio::sync::Semaphore;
// Local Imports

/* === Constants === */

// The lane resident controls run in: unbounded, so a control that runs until
// cancelled never holds a slot another call is waiting for.
pub(crate) const RESIDENT: &str = "harmos.resident";

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Lane {
    name: &'static str,
    capacity: Capacity,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Capacity {
    width: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LaneSettings {
    pub capacity: Capacity,
}

#[derive(Clone, Debug)]
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

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl Lane {
    pub const fn new(name: &'static str, capacity: Capacity) -> Self {
        Self { name, capacity }
    }
    pub const fn name(self) -> &'static str {
        self.name
    }
    pub const fn capacity(self) -> Capacity {
        self.capacity
    }
}

impl Default for Lane {
    fn default() -> Self {
        Self::new("Open", Capacity::unbounded())
    }
}

impl Default for LaneSet {
    fn default() -> Self {
        let lane = Lane::default();
        Self {
            entries: vec![(lane.name(), lane.capacity())],
        }
    }
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
    pub fn new(lanes: impl IntoIterator<Item = Lane>) -> crate::Result<Self> {
        let mut set = Self::default();
        for lane in lanes {
            if lane.name.is_empty() || lane.name.chars().any(char::is_control) || lane.name == RESIDENT {
                return Err(crate::Error::InvalidLane {
                    name: lane.name,
                    reason: "names must be nonempty, contain no control characters, and not be reserved",
                });
            }
            if lane.capacity.width() == 0 {
                return Err(crate::Error::InvalidLane {
                    name: lane.name,
                    reason: "must admit at least one job",
                });
            }
            if set.contains(lane.name) {
                return Err(crate::Error::InvalidLane {
                    name: lane.name,
                    reason: "declared more than once",
                });
            }
            set.entries.push((lane.name, lane.capacity));
        }
        Ok(set)
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

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
