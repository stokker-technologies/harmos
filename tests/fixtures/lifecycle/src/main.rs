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
#[derive(clap::Parser)]
struct Cli {
    #[command(flatten)]
    serve: harmos_lifecycle_fixture::serve::Args,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

/* === Private Functions === */
#[tokio::main]
async fn main() -> harmos_lifecycle_fixture::Result<()> {
    let args = <Cli as clap::Parser>::parse();
    harmos_lifecycle_fixture::serve::run(args.serve).await
}

/* === Tests === */
