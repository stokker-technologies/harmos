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
use std::num::NonZeroUsize;

// Third Party Imports
use harmos::prelude::*;
use harmos_protocol::host;
use schematic_protocol::{ValidateSchematic, Validation};
use tonic::{Request, transport::Endpoint};

// Local Imports

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

#[tokio::main(flavor = "current_thread")]
async fn main() -> std::result::Result<(), Box<dyn std::error::Error>> {
    // Connect To The Application's gRPC Listener.
    let address = std::env::args().nth(1).ok_or("pass the application's gRPC URL")?;
    let channel = Endpoint::from_shared(address)?.connect().await?;
    let retained_bytes = std::env::var("HARMOS_CLIENT_RETAINED_BYTES")?.parse::<NonZeroUsize>()?;
    let settings = ClientSettings {
        retained_bytes,
        message_bytes: NonZeroUsize::new(1024 * 1024).unwrap(),
        input_items: NonZeroUsize::new(4).unwrap(),
        input_bytes: NonZeroUsize::new(1024 * 1024).unwrap(),
    };
    let client = Client::new(channel, settings)?;

    // Find The Application's Generated Contract In Bounded Discovery Pages.
    let mut page_token = String::new();
    let contract = loop {
        let request = host::CatalogEntriesListRequest {
            kind: host::CatalogKind::Operations as i32,
            page: Some(harmos_protocol::v1::PageRequest {
                page_size: 64,
                max_bytes: 1024 * 1024,
                page_token,
            }),
            ..Default::default()
        };
        let page = client.catalog(Request::new(request)).await?.into_inner();
        if let Some(contract) =
            page.operations.into_iter().filter_map(|entry| entry.contract).find(|contract| contract.id == "schematic.validate")
        {
            break contract;
        }
        if page.next_page_token.is_empty() {
            return Err("the host does not expose schematic.validate".into());
        }
        page_token = page.next_page_token;
    };

    // Bind Generated Application Messages And Observe The Operation's Status.
    let method = Method::<ValidateSchematic, Validation>::new(contract)?;
    let request = ValidateSchematic {
        schematic: Some(schematic_protocol::Snapshot {
            symbols: vec![1, 2],
            connections: vec![schematic_protocol::Connection { from: 1, to: 2 }],
        }),
    };
    match client.call(&method, Request::new(request)).await {
        Ok(response) => println!("{:?}", response.into_inner()),
        Err(status) => println!("{}: {}", status.code(), status.message()),
    }
    Ok(())
}

/* === Private Functions === */

/* === Tests === */
