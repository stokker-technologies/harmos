/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Generated Application Client

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::{env, error::Error as StdError, num::NonZeroUsize, result::Result as StdResult};

// Third Party Imports
use harmos::client::Method;
use harmos::{Client, ClientSettings, StorageTransferSettings};
use schematic_protocol::{Connection, Snapshot, ValidateSchematic, Validation};

// Local Imports

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

#[tokio::main]
async fn main() -> StdResult<(), Box<dyn StdError>> {
    // Connect to the Application's gRPC Listener.
    let address = env::args().nth(1).ok_or("pass the application's gRPC URL")?;
    let retained_bytes = env::var("HARMOS_CLIENT_RETAINED_BYTES")?.parse::<NonZeroUsize>()?;
    let settings = ClientSettings {
        transfer: StorageTransferSettings { chunk_bytes: Some(512 * 1024) },
        retained_bytes,
        message_bytes: NonZeroUsize::new(1024 * 1024).unwrap(),
        input_items: NonZeroUsize::new(4).unwrap(),
        input_bytes: NonZeroUsize::new(1024 * 1024).unwrap(),
    };
    let client = Client::connect(address, settings).await?;

    // Find the Application's Generated Contract.
    let declarations = client.operations.list().await?;
    let contract = declarations
        .into_iter()
        .filter_map(|entry| entry.contract)
        .find(|contract| contract.id == "schematic.validate")
        .ok_or("the host does not expose schematic.validate")?;

    // Bind Generated Application Messages and Observe the Operation's Status.
    let method = Method::<ValidateSchematic, Validation>::new(contract)?;
    let request = ValidateSchematic {
        schematic: Some(Snapshot {
            symbols: vec![1, 2],
            connections: vec![Connection { from: 1, to: 2 }],
        }),
    };
    match client.call(&method, request).await {
        Ok(response) => println!("{:?}", response),
        Err(status) => println!("{}: {}", status.code(), status.message()),
    }
    Ok(())
}

/* === Private Functions === */

/* === Tests === */
