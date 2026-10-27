/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Re-Exports === */

pub use harmos_operations::lifecycle::{Cancel, CancelRequest, InstanceState, Status, StatusRequest, Watch, WatchRequest};

/* === Structs === */

pub struct Catalog;

/* === Implementations === */

impl Catalog {
    pub fn registration() -> harmos_sdk::RegistrationBatch {
        harmos_operations::lifecycle::Lifecycle::registration(harmos_sdk::Dependencies::new())
    }
}
