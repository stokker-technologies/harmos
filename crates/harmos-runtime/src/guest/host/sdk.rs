/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

mod bindings {
    wasmtime::component::bindgen!({
        path: "../harmos-guest/wit",
        world: "plugin",
        exports: { default: async | store },
    });
}

// Both Phases And SDK Calls Use The Original Store's Async Operation Function
pub(super) use bindings::{Plugin, PluginIndices, harmos::guest::capabilities};
