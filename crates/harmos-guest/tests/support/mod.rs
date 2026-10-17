/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

#![allow(dead_code)]
/* === Modules === */
#[path = "../../../../tests/support/fixtures.rs"]
mod fixtures;

/* === Re-Exports === */

/* === Imports === */
// Standard Library Imports
use fixtures::component as load;
use std::sync::OnceLock;
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
pub fn plugin_component() -> Vec<u8> {
    static COMPONENT: OnceLock<Vec<u8>> = OnceLock::new();
    COMPONENT
        .get_or_init(|| load("plugin-component", "componentize,kit", None))
        .clone()
}

/* === Private Functions === */
/* === Tests === */
