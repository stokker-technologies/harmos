/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */


// Await Native Resident Status Events Without A Polling Timer.

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

/* === Public Functions === */

/* === Private Functions === */
// Await The Resident Status Change The Story Names; The Lane Deadline Ends A Hang.
pub(super) async fn service_status(
    mut services: tokio::sync::watch::Receiver<BTreeMap<harmos_runtime::operations::resident::Key, harmos_runtime::operations::resident::Snapshot>>,
    name: &str, wanted: impl Fn(&ServiceStatus) -> bool,
) -> ServiceStatus {
    let key = harmos_runtime::operations::resident::Key::root(name);
    let snapshot = services
        .wait_for(|items| items.get(&key).and_then(|item| item.health.as_ref()).is_some_and(&wanted))
        .await
        .expect("the runtime keeps the status sender open");
    snapshot.get(&key).and_then(|item| item.health.clone()).expect("the awaited status is present")
}



/* === Tests === */
