/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */

#[cfg(all(not(target_arch = "wasm32"), feature = "grpc", feature = "streaming"))]
mod append;

/* === Re-Exports === */

#[cfg(all(not(target_arch = "wasm32"), feature = "grpc", feature = "streaming"))]
pub use append::Append;
pub use harmos_protocol::v1::FollowingRow;

/* === Imports === */

// Standard Library Imports

// Third Party Imports

// Local Imports

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

#[cfg(all(not(target_arch = "wasm32"), feature = "grpc", feature = "streaming"))]
pub fn streams_catalog() -> crate::execution::operations::Catalog {
    streams_catalog_batch().named("harmos.streams")
}

#[cfg(all(not(target_arch = "wasm32"), feature = "grpc", feature = "streaming"))]
pub fn streams_catalog_batch() -> crate::execution::__private::Catalog {
    let mut batch = crate::execution::__private::Catalog::new();
    batch.operations.push(crate::execution::operations::OperationFactory::protobuf::<Append>().expect("Append declaration"));
    batch
}

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

// Append Through the Native Declaration With the Caller's Authority.
#[cfg(all(not(target_arch = "wasm32"), feature = "grpc", feature = "streaming"))]
pub async fn publish_on<H, S>(host: &H, row: &S::Row) -> Result<(), H::Error>
where
    H: crate::execution::operations::ProtobufHost,
    S: harmos_operations::StreamSpec,
    S::Row: harmos_domain::Message,
{
    host.call_protobuf::<Append>(harmos_protocol::context::SamplesAppendRequest {
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
