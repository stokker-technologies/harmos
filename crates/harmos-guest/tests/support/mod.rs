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

#[path = "../../../harmos-runtime/tests/support/fixtures.rs"]
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
    COMPONENT.get_or_init(|| load("schematic-guest")).clone()
}

pub fn sandbox() -> Vec<u8> {
    static COMPONENT: OnceLock<Vec<u8>> = OnceLock::new();
    COMPONENT.get_or_init(|| load("harmos-sandbox-guest")).clone()
}

/* === Private Functions === */

/* === Tests === */
