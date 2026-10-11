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

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

/* === Private Functions === */
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let arguments: Vec<_> = std::env::args_os().skip(1).collect();
    let [input, output] = arguments.as_slice() else {
        return Err("usage: harmos-component-fixture <module.wasm> <component.wasm>".into());
    };
    let module = std::fs::read(input)?;
    let component = harmos_guest::componentize(&module)?;
    std::fs::write(output, component)?;
    Ok(())
}

/* === Tests === */
