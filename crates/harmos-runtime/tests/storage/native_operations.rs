/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Private Storage Operation Journeys

/* === Modules === */

mod expression_transforms_through_list_rows;
mod native_row_pages_replay_and_keep_their_durable_image;
mod native_row_watch_delivers_durable_rows_and_settles_cancellation;
mod native_upload_resumes_fences_writers_and_preserves_read_cuts;

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports

// Third Party Imports

// Local Imports
use super::{Fixture, config, storage_catalog};
use crate::{storage, support::app};
use harmos_runtime::Identifier;
use harmos_runtime::operations::Lane;
use harmos_runtime::storage::MutationId;
use harmos_runtime::storage::query::{RecordSelection, Source};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

/* === Private Functions === */

async fn fixture() -> Fixture {
    let fixture = Fixture::new(config()).await;
    app::register(&fixture.runtime, storage_catalog(), Lane::default()).await.unwrap();
    fixture
}
fn mutation(fixture: &Fixture, key: &str) -> MutationId {
    MutationId {
        window: fixture.storage.mutation_window().unwrap().id,
        key: Identifier::new(key).unwrap(),
    }
}
fn records() -> Source {
    Source::Records(RecordSelection {
        schema: storage::schema(),
        scope: None,
        item: None,
        source_start: None,
        source_end: None,
    })
}

/* === Tests === */
