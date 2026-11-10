/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Run the Reference Host Against Any Language SDK.

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports

// Third Party Imports

// Local Imports
use harmos_conformance::{Error, Target};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

#[tokio::main]
async fn main() {
    let result = async {
        let mut arguments = std::env::args().skip(1);
        let mode = arguments.next();
        if mode.as_deref() == Some("--help") {
            println!("Harmos Conformance\nUsage: harmos-conformance --address ADDRESS\n       harmos-conformance --plugin EXECUTABLE [-- ARGUMENTS...]");
            return Ok(true);
        }
        let value = arguments.next().ok_or_else(|| Error::Invalid("Expected --address ADDRESS or --plugin EXECUTABLE".into()))?;
        let rest = arguments.collect::<Vec<_>>();
        let target = match mode.as_deref() {
            Some("--address") if rest.is_empty() => Target::Address(value),
            Some("--plugin") if rest.is_empty() => Target::Executable { path: value.into(), arguments: rest },
            Some("--plugin") if rest.first().map(String::as_str) == Some("--") => Target::Executable { path: value.into(), arguments: rest.into_iter().skip(1).collect() },
            _ => return Err(Error::Invalid("Expected --address ADDRESS or --plugin EXECUTABLE [-- ARGUMENTS...]".into())),
        };
        harmos_conformance::run(target).await
    }.await;
    match result {
        Ok(true) => {}
        Ok(false) => std::process::exit(1),
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(2);
        }
    }
}

/* === Private Functions === */

/* === Tests === */
