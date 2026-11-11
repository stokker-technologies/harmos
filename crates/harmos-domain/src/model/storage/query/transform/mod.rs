/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */

pub mod aggregate;
pub mod downsample;
pub mod filter;
pub mod histogram;
pub mod project;
pub mod sort;

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports

// Third Party Imports

// Local Imports
use crate::MemorySize;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FieldPath {
    pub fields: Vec<String>,
}

/* === Enums === */

#[derive(Clone, Debug)]
pub enum Transform {
    Filter(filter::Filter),
    Project(project::Project),
    Aggregate(aggregate::Aggregate),
    Histogram(histogram::Histogram),
    Downsample(downsample::Downsample),
    Sort(sort::Sort),
}

/* === Traits === */

/* === Implementations === */

impl From<&str> for FieldPath {
    fn from(name: &str) -> Self {
        Self {
            fields: name.split('.').map(str::to_owned).collect(),
        }
    }
}
impl Transform {
    pub fn is_streaming(&self) -> bool {
        matches!(self, Self::Filter(_) | Self::Project(_))
    }
}
impl MemorySize for FieldPath {
    fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>() + self.fields.memory_size().saturating_sub(std::mem::size_of_val(&self.fields))
    }
}
impl MemorySize for Transform {
    fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>()
            + match self {
                Self::Filter(value) => value.memory_size().saturating_sub(std::mem::size_of_val(value)),
                Self::Project(value) => value.memory_size().saturating_sub(std::mem::size_of_val(value)),
                Self::Aggregate(value) => value.memory_size().saturating_sub(std::mem::size_of_val(value)),
                Self::Histogram(value) => value.memory_size().saturating_sub(std::mem::size_of_val(value)),
                Self::Downsample(value) => value.memory_size().saturating_sub(std::mem::size_of_val(value)),
                Self::Sort(value) => value.memory_size().saturating_sub(std::mem::size_of_val(value)),
            }
    }
}

/* === Public Functions === */

pub fn invalid(reason: &'static str) -> crate::Error {
    crate::Error::InvalidInput { field: "storage query transform", reason }
}

/* === Private Functions === */

/* === Tests === */
