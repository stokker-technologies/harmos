/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Call the Generated Schematic Client

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::{env, error::Error, num::NonZeroUsize};

// Third Party Imports

// Local Imports
use harmos::{Client, ClientSettings, StorageTransferSettings};
use schematic_protocol::{Connection, Snapshot, client::Client as SchematicClient};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

/* === Private Functions === */

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    // Connect With the Application's Decoded Memory Budget.
    let address = env::args().nth(1).ok_or("Connect Schematic Client: Supply the gRPC URL")?;
    let retained_bytes = env::var("HARMOS_CLIENT_RETAINED_BYTES")?.parse::<NonZeroUsize>()?;
    let settings = ClientSettings::for_storage(StorageTransferSettings::default(), retained_bytes)?;
    let library = Client::connect(address, settings).await?;
    let client = SchematicClient::new(library.transport_handle());

    // Validate Through the Application's Generated Resource.
    let snapshot = Snapshot {
        symbols: vec![1, 2],
        connections: vec![Connection { from: 1, to: 2 }],
    };
    let diagnostics = client.schematic.validate(snapshot).await?;
    println!("{diagnostics:?}");
    Ok(())
}

/* === Tests === */
