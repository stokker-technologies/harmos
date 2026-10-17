/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Report Independent Contract Variants Within Integration Scenarios

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

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */
pub fn check(name: &str, scenario: impl FnOnce()) {
    eprintln!("Contract Variant: {name}");
    scenario();
}

/* === Private Functions === */

/* === Tests === */
