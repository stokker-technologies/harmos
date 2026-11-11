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
mod publish;

/* === Re-Exports === */

pub use harmos_protocol::host::FollowingRow;
#[cfg(all(not(target_arch = "wasm32"), feature = "grpc", feature = "streaming"))]
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

#[cfg(all(not(target_arch = "wasm32"), feature = "grpc", feature = "streaming"))]
pub struct Streams;

#[cfg(all(not(target_arch = "wasm32"), feature = "grpc", feature = "streaming"))]
impl crate::execution::operations::OperationCatalog for Streams {
    const CATALOG: crate::execution::operations::Catalog = crate::execution::operations::Catalog::of::<Self>("harmos.streams", &[], &[]);
    fn batch() -> crate::execution::__private::RegistrationBatch {
        let mut batch = crate::execution::__private::RegistrationBatch::new();
        batch.operations.push(crate::execution::operations::OperationFactory::protobuf::<Publish>().expect("Publish declaration"));
        batch
    }
}
#[cfg(all(not(target_arch = "wasm32"), feature = "grpc", feature = "streaming"))]
impl Streams {
    pub fn catalog() -> Vec<crate::OperationDefinition> {
        <Self as crate::execution::operations::OperationCatalog>::batch().operations.into_iter().map(|factory| factory.definition).collect()
    }
    pub const CATALOG: crate::execution::operations::Catalog = <Self as crate::execution::operations::OperationCatalog>::CATALOG;
}

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

// Publish Through the Native Declaration With the Caller's Authority.
#[cfg(all(not(target_arch = "wasm32"), feature = "grpc", feature = "streaming"))]
pub async fn publish_on<H, S>(host: &H, row: &S::Row) -> Result<(), H::Error>
where
    H: crate::execution::operations::ProtobufHost,
    S: harmos_domain::StreamSpec,
    S::Row: harmos_domain::Message,
{
    host.call_protobuf::<Publish>(harmos_protocol::host::Publish {
        stream: S::ID.to_owned(),
        sequence: None,
        published_unix_micros: None,
        context: None,
        payload: harmos_domain::Message::encode(row),
        following: Vec::new(),
    })
    .await?;
    Ok(())
}

/* === Private Functions === */

/* === Tests === */
