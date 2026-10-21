/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */

mod bindings {
    wasmtime::component::bindgen!({
        path: "../harmos-guest/wit",
        world: "plugin",
        exports: { default: async | store },
    });
}

/* === Re-Exports === */

// Both Phases And SDK Calls Use The Original Store's Async Operation Function
pub(super) use bindings::{Plugin, PluginIndices, harmos::guest::capabilities};

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

/* === Private Functions === */

/* === Tests === */
