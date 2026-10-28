/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Storage Arrow Query Contract

use super::FieldPath;
use crate::MemorySize;
use crate::model::storage::codec::{Float, Many, Nested, product};

product!(Histogram { field: Nested<FieldPath> => 1, edges: Many<Float> => 2 });

#[derive(Clone, Debug)]
pub struct Histogram {
    pub field: FieldPath,
    pub edges: Vec<f64>,
}

impl MemorySize for Histogram {
    fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>()
            + self.field.memory_size().saturating_sub(std::mem::size_of_val(&self.field))
            + self.edges.capacity().saturating_mul(8)
    }
}
