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
pub fn component() -> Vec<u8> {
    static COMPONENT: OnceLock<Vec<u8>> = OnceLock::new();
    COMPONENT
        .get_or_init(|| load("test-component", "test-component", None))
        .clone()
}

pub fn route_component() -> Vec<u8> {
    static COMPONENT: OnceLock<Vec<u8>> = OnceLock::new();
    COMPONENT
        .get_or_init(|| load("route-component", "test-component", None))
        .clone()
}

pub fn legacy_component() -> Vec<u8> {
    static COMPONENT: OnceLock<Vec<u8>> = OnceLock::new();
    COMPONENT
        .get_or_init(|| load("legacy-component", "componentize,kit", None))
        .clone()
}

pub fn plugin_component() -> Vec<u8> {
    static COMPONENT: OnceLock<Vec<u8>> = OnceLock::new();
    COMPONENT
        .get_or_init(|| load("plugin-component", "componentize,kit", None))
        .clone()
}

/* === Private Functions === */
/* === Tests === */
