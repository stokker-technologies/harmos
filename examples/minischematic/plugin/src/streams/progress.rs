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
pub use minischematic_contract::autoroute::Progress;

/* === Imports === */
// Standard Library Imports

// Third Party Imports

// Local Imports

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */
pub async fn publish(context: &harmos::guest::Context, percent: u8) -> crate::Result<()> {
    context
        .publisher::<crate::Autoroute>()
        .publish_on::<Progress>(&Progress {
            percent: u32::from(percent),
        })
        .await
        .map_err(crate::Error::from)
}

/* === Private Functions === */

/* === Tests === */
