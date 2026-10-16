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
use harmos::storage::operations::{Operations, Save, SaveRequest};

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

pub(super) fn save_request(
    fixture: &Fixture,
    through: harmos::storage::StorageCursor,
) -> SaveRequest {
    SaveRequest {
        mutation: harmos::storage::MutationId {
            window: fixture.storage.mutation_window().unwrap().id,
            key: harmos::Identifier::new("save").unwrap(),
        },
        at: through,
    }
}

/* === Tests === */
