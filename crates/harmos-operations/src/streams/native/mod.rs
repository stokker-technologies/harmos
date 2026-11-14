/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Native Publication Operations

/* === Modules === */

mod publish;

/* === Re-Exports === */

use super::PublishRequest;
pub use publish::Publish;

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

pub fn streams_catalog() -> crate::execution::operations::Catalog {
    streams_catalog_batch().named("harmos.streams")
}

pub fn streams_catalog_batch() -> crate::execution::__private::Catalog {
    let mut batch = crate::execution::__private::Catalog::new();
    batch.operations.push(crate::execution::operations::OperationFactory::native::<Publish>().expect("Append declaration"));
    batch
}

// Append Through the Native Declaration With the Caller's Authority.
pub async fn publish_on<H, S>(host: &H, row: &S::Row) -> Result<(), H::Error>
where
    H: crate::execution::operations::NativeHost,
    S: harmos_operations::StreamSpec,
    S::Row: harmos_domain::Message,
{
    host.call_native::<Publish>(PublishRequest {
        stream: S::ID.to_owned(),
        sequence: None,
        published_at: None,
        context: None,
        payload: harmos_domain::Message::encode(row),
        following: Vec::new(),
    })
    .await?;
    Ok(())
}

/* === Private Functions === */

/* === Tests === */
