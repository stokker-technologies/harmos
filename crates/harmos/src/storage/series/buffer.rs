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
impl<T: MemorySize + 'static> Series<T> {
    pub(super) async fn collect(&mut self) -> Result<()> {
        if self.complete {
            self.view()?.saved()?;
            return Ok(());
        }
        while let Some(page) = {
            let remaining = self.max_samples.get().saturating_sub(self.samples);
            let projection = &self.project;
            self.query
                .next_page_projected_including_empty(
                    self.query.decoded_bytes(),
                    |rows, allowance| {
                        if rows.len() > remaining {
                            return Err(invalid("series samples exceed explicit bound"));
                        }
                        let bytes = std::mem::size_of::<Vec<SeriesPoint>>().saturating_add(
                            rows.len()
                                .saturating_mul(std::mem::size_of::<SeriesPoint>()),
                        );
                        if bytes > allowance.get() {
                            return Err(invalid("series page output exceeds decoded allowance"));
                        }
                        let mut points = Vec::new();
                        points
                            .try_reserve_exact(rows.len())
                            .map_err(|_| invalid("series point allocation failed"))?;
                        if points.capacity() != rows.len() {
                            return Err(invalid("series point capacity exceeds reservation"));
                        }
                        for row in rows {
                            let point = projection(row, allowance)?;
                            point.axis.check()?;
                            point.value.check()?;
                            if point.axis.kind() != Some(self.source.columns[0].kind)
                                || point.value.kind() != Some(self.source.columns[1].kind)
                            {
                                return Err(invalid(
                                    "series projection types differ from selected fields",
                                ));
                            }
                            points.push(point);
                        }
                        Ok(points)
                    },
                )
                .await?
        } {
            if page.is_empty() {
                self.complete = true;
                return Ok(());
            }
            // Store The Original Loan Before The Next Await: Cancellation Can
            // Resume The Same Pages And Continuation Without Losing Samples.
            self.samples += page.len();
            self.pages.push(page);
        }
        self.complete = true;
        Ok(())
    }

    pub(super) fn points(&self) -> impl Iterator<Item = &SeriesPoint> {
        self.pages.iter().flat_map(|page| page.iter())
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
