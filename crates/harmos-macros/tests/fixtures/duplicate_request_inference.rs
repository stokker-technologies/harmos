/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */

#[path = "policy.rs"]
mod controls;

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports

// Third Party Imports

// Local Imports

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

#[harmos::message]
struct Request {}

#[harmos::operation(request = Request, response = (), policy = controls::POLICY, infer_request)]
struct Read;

#[harmos::operation(request = Request, response = (), policy = controls::POLICY, infer_request)]
struct AlsoRead;

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

/* === Private Functions === */

fn main() {}

/* === Tests === */
