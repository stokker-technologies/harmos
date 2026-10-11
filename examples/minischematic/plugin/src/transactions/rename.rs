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
use harmos::operations::host;
use minischematic_contract::autoroute::{NET_RENAME, RenameNet};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */
pub async fn rename(
    context: &harmos::guest::Context,
    change: &RenameNet,
) -> crate::Result<host::Receipt> {
    let payload = serde_json::to_vec(change)?;
    context
        .operation::<host::Invoke>(host::InvokeRequest {
            grant: NET_RENAME.to_owned(),
            key: "route-vcc".to_owned(),
            payload,
        })
        .await
        .map_err(crate::Error::from)
}

/* === Private Functions === */

/* === Tests === */
