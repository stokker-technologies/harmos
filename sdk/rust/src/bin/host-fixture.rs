/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Managed Host Lifecycle Fixture

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports

// Third Party Imports

// Local Imports
use harmos_sdk_grpc::{Context, Message, Operation, Plugin, Result};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

#[derive(Message)]
struct Input {
    value: Vec<u8>,
}

#[derive(Message)]
struct Output {
    value: Vec<u8>,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

/* === Private Functions === */

fn main() -> Result<()> {
    let cleanup = std::env::args().nth(1).expect("Cleanup Evidence Path");
    let plugin = Plugin::new("placement", "1.0.0").operation(Operation::new("placement.echo", echo)).on_stop(move || {
        std::fs::write(&cleanup, b"stopped")?;
        Ok(())
    });
    plugin.serve()
}

async fn echo(_: &Context, input: Input) -> Result<Output> {
    Ok(Output { value: input.value })
}

/* === Tests === */
