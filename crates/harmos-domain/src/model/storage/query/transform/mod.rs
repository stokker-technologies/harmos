/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Storage Arrow Query Contract

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
use crate::model::storage::codec::{Many, Nested, Text, product, variants};

/* === Constants === */

/* === Macros === */
product!(FieldPath { fields: Many<Text> => 1 });
variants!(Transform {
    0, 1, "filter": Self::Filter(value) => { value: Nested<filter::Filter> => 1 } => Self::Filter(value);
    1, 2, "project": Self::Project(value) => { value: Nested<project::Project> => 1 } => Self::Project(value);
    2, 3, "aggregate": Self::Aggregate(value) => { value: Nested<aggregate::Aggregate> => 1 } => Self::Aggregate(value);
    3, 4, "histogram": Self::Histogram(value) => { value: Nested<histogram::Histogram> => 1 } => Self::Histogram(value);
    4, 5, "downsample": Self::Downsample(value) => { value: Nested<downsample::Downsample> => 1 } => Self::Downsample(value);
    5, 6, "sort": Self::Sort(value) => { value: Nested<sort::Sort> => 1 } => Self::Sort(value);
});

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
        std::mem::size_of::<Self>()
            + self
                .fields
                .memory_size()
                .saturating_sub(std::mem::size_of_val(&self.fields))
    }
}
impl MemorySize for Transform {
    fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>()
            + match self {
                Self::Filter(value) => value
                    .memory_size()
                    .saturating_sub(std::mem::size_of_val(value)),
                Self::Project(value) => value
                    .memory_size()
                    .saturating_sub(std::mem::size_of_val(value)),
                Self::Aggregate(value) => value
                    .memory_size()
                    .saturating_sub(std::mem::size_of_val(value)),
                Self::Histogram(value) => value
                    .memory_size()
                    .saturating_sub(std::mem::size_of_val(value)),
                Self::Downsample(value) => value
                    .memory_size()
                    .saturating_sub(std::mem::size_of_val(value)),
                Self::Sort(value) => value
                    .memory_size()
                    .saturating_sub(std::mem::size_of_val(value)),
            }
    }
}

/* === Public Functions === */
pub fn invalid(reason: &'static str) -> crate::Error {
    crate::Error::InvalidInput {
        field: "storage query transform",
        reason,
    }
}

/* === Private Functions === */

/* === Tests === */
