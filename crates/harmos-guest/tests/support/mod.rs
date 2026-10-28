/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

#![allow(dead_code)]

#[path = "../../../harmos-runtime/tests/support/fixtures.rs"]
mod fixtures;

use fixtures::component as load;
use std::sync::OnceLock;

pub fn component() -> Vec<u8> {
    static COMPONENT: OnceLock<Vec<u8>> = OnceLock::new();
    COMPONENT.get_or_init(|| load("schematic-guest")).clone()
}

pub fn sandbox() -> Vec<u8> {
    static COMPONENT: OnceLock<Vec<u8>> = OnceLock::new();
    COMPONENT.get_or_init(|| load("harmos-sandbox-guest")).clone()
}
