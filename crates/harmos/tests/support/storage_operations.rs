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
use std::num::NonZeroUsize;
use std::sync::Arc;

// Third Party Imports

// Local Imports
use super::{Fixture, config, write};
use harmos::operations::Contracts;
use harmos::storage::operations::Operations;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */
#[harmos::lanes]
pub(super) enum Lanes {
    #[default]
    Open,
}

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

/* === Private Functions === */
fn followed(
    item: harmos::operations::StreamItem<harmos::storage::operations::Followed>,
) -> harmos::storage::StoredRecord {
    match &*item {
        harmos::storage::operations::Followed::Record(record) => record.clone(),
        harmos::storage::operations::Followed::Boundaries(_) => {
            panic!("boundaries are reported once, before records")
        }
    }
}

/* === Tests === */
