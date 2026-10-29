/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

mod reduce;
mod series;
mod summary;

pub use reduce::{decimate, decimate_indices, extrema, lttb, lttb_log};
pub use series::Series;
pub use summary::Summary;
