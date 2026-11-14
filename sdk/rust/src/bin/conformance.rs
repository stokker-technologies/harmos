/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */

#[path = "../../tests/fixtures/conformance/mod.rs"]
pub mod conformance;
mod install_itself;

/* === Re-Exports === */

/* === Imports === */

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

/* === Private Functions === */

fn main() -> harmos::Result<()> {
    let arguments: Vec<_> = std::env::args().skip(1).collect();
    if arguments.first().is_some_and(|argument| argument == "--install-itself") {
        return install_itself::run(&arguments[1..]);
    }
    let plugin = conformance::plugin();
    if arguments.iter().any(|argument| argument == "--remote") {
        plugin.remote().serve()
    } else {
        plugin.serve()
    }
}

/* === Tests === */
