/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Test-Only Coordinate Cache And Selection Access

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

pub struct Coordinates(super::Coordinates);

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl Coordinates {
    pub fn new(batch: &RecordBatch, plan: &Downsample, cached: bool) -> Result<Self> {
        let mut builder = Builder::cached(batch.schema())?;
        builder.push(batch, &mut || Ok(()))?;
        let spool = builder.finish()?;
        let budget = if cached { COORDINATE_CACHE_BYTES } else { 0 };
        super::coordinates(&spool, plan, budget, &mut || Ok(())).map(Self)
    }

    pub fn is_cached(&self) -> bool {
        self.0.file.is_none()
    }

    pub fn get(&mut self, index: usize) -> Result<(f64, f64)> {
        self.0.get(index)
    }

    pub fn select(&mut self, plan: &Downsample) -> Result<Vec<usize>> {
        let mut selected = Selection {
            file: tempfile::tempfile().map_err(io)?,
            last: None,
        };
        triangles(&mut self.0, plan, &mut selected, &mut || Ok(()))?;
        selected.file.rewind().map_err(io)?;
        let mut indices = Vec::new();
        while let Some(index) = read_index(&mut selected.file)? {
            indices.push(index);
        }
        Ok(indices)
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
