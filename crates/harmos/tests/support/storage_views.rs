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

/* === Public Functions === */

/* === Private Functions === */
fn limits(rows: usize, bytes: usize) -> ReadLimits {
    ReadLimits {
        rows: NonZeroUsize::new(rows).unwrap(),
        bytes: NonZeroUsize::new(bytes).unwrap(),
    }
}

async fn positions(mut reader: RecordQueryResult<Vec<u8>>) -> Vec<u64> {
    let mut result = Vec::new();
    while let Some(page) = reader.next_page().await.unwrap() {
        result.extend(page.iter().map(|row| row.position.sequence));
    }
    result
}

pub(super) async fn wait_release(f: &Fixture, count: usize) {
    tokio::time::timeout(Duration::from_secs(1), async {
        while f.data.control.releases.load(Ordering::SeqCst) < count {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
}

/* === Tests === */
