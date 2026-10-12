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
use wit_component::ComponentEncoder;
// Local Imports

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */
pub fn componentize(module: &[u8]) -> Result<Vec<u8>, String> {
    ComponentEncoder::default()
        .module(module)
        .map_err(|error| format!("{error:#}"))?
        .validate(true)
        .encode()
        .map_err(|error| format!("{error:#}"))
}

/* === Private Functions === */

/* === Tests === */
