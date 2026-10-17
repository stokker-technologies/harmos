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

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */
#[harmos_domain::message]
#[derive(Clone, Debug, PartialEq)]
pub struct Point {
    pub x: i32,
    pub y: i32,
}

#[harmos_domain::message]
#[derive(Clone, Debug, PartialEq)]
pub struct Connection {
    pub from: u64,
    pub to: u64,
}

#[harmos_domain::message]
#[derive(Clone, Debug, PartialEq)]
pub struct Snapshot {
    pub symbols: Vec<u64>,
    pub connections: Vec<Connection>,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
