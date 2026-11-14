/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Library Client

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::{env, error::Error as StdError, num::NonZeroUsize, result::Result as StdResult};

// Third Party Imports
use harmos::{Client, ClientSettings, StorageTransferSettings};

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

    // Discover the Library Operations and Read Host Time.
    let operations = client.operations.list().await?;
    println!("{} operations", operations.len());
    let now = client.time.get().await?;
    println!("Host time: {}", now.seconds);
    Ok(())
}

/* === Private Functions === */

/* === Tests === */
