/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Access Native Bucket Checkpoints

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports

// Third Party Imports

// Local Imports
use super::*;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl Sample {
    pub(crate) fn memory_bytes(&self) -> usize {
        std::mem::size_of::<Self>() + self.identity.capacity() + self.key.capacity()
    }
}

impl Accumulator {
    pub(crate) fn into_samples(self) -> Vec<Sample> {
        self.samples
    }

    pub(crate) fn from_samples(plan: &TimeBucketPlan, input: Arc<Schema>, samples: Vec<Sample>) -> Result<Self> {
        let mut accumulator = Self::new(plan, input)?;
        accumulator.owned_bytes = samples.iter().map(|sample| sample.identity.capacity() + sample.key.capacity()).sum();
        accumulator.samples = samples;
        if accumulator.samples.len() > plan.max_samples.get() || accumulator.memory_bytes() > plan.max_bytes.get() {
            return Err(columnar::exhausted());
        }
        Ok(accumulator)
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
