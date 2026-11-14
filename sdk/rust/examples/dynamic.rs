/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Dynamic Fixture Invocation

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
    // Connect to a Host Running the Shared Conformance Fixture.
    let address = env::args().nth(1).ok_or("pass the fixture host's gRPC URL")?;
    let settings = ClientSettings {
        transfer: StorageTransferSettings { chunk_bytes: Some(512 * 1024) },
        retained_bytes: env::var("HARMOS_CLIENT_RETAINED_BYTES")?.parse::<NonZeroUsize>()?,
        message_bytes: NonZeroUsize::new(1024 * 1024).unwrap(),
        input_items: NonZeroUsize::new(4).unwrap(),
        input_bytes: NonZeroUsize::new(1024 * 1024).unwrap(),
    };
    let client = Client::connect(address, settings).await?;

    // Discover the Fixture Contract and Pass an Encoded Value Without Generated Types.
    let operation = client
        .operations
        .list()
        .await?
        .into_iter()
        .filter_map(|entry| entry.contract)
        .find(|contract| contract.id.ends_with("conformance.step.0"))
        .ok_or("the host does not expose conformance.step.0")?;
    // Value { value: b"hello" }, declared in sdk/tests/fixtures/conformance/proto/.
    let payload = vec![0x0a, 0x05, b'h', b'e', b'l', b'l', b'o'];
    let invocation = client.invocations.create(operation.id, operation.fingerprint, payload).await?;
    println!("{:?}", invocation);
    Ok(())
}

/* === Private Functions === */

/* === Tests === */
