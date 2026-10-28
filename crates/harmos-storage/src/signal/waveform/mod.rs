/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

mod calculus;
mod edge;
mod reduce;
mod resample;
mod series;
mod summary;

pub use calculus::{derivative, integral};
pub use edge::{Edge, crossing, edges};
pub use reduce::{above_floor, decimate, decimate_indices, extrema, lttb, lttb_log};
pub use resample::{nearest, resample, resample_count, resample_uniform, value_at};
pub use series::Series;
pub use summary::Summary;
