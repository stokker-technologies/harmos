/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */

#[path = "../support/native_streams/plugin.rs"]
mod plugin;

/* === Public Functions === */

fn main() -> harmos::Result<()> {
    plugin::plugin().serve()
}
