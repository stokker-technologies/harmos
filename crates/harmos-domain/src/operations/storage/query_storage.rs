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
use crate::model::storage::StorageInfo;
use crate::operations::{OperationDeclaration, ReplaySafety, Visibility, WireOperation};

/* === Constants === */

/* === Macros === */
crate::postcard_product!(QueryStorageRequest {});

/* === Type Aliases === */

/* === Structs === */
pub struct QueryStorage;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct QueryStorageRequest;

/* === Enums === */

/* === Traits === */

/* === Implementations === */
impl OperationDeclaration for QueryStorage {
    type Request = QueryStorageRequest;
    type Response = StorageInfo;
    const ID: &'static str = "storage.query_storage";
    const POLICY: crate::operations::OperationPolicy = super::POLICY;
    const VISIBILITY: Visibility = Visibility::Host;
    const REPLAY: ReplaySafety = ReplaySafety::Idempotent;
}

impl WireOperation for QueryStorage {
    const VERSION: u32 = 1;
}

impl crate::MemorySize for QueryStorageRequest {
    fn memory_size(&self) -> usize {
        0
    }
}

impl crate::schema::Described for QueryStorageRequest {
    const NAME: &'static str = concat!(module_path!(), "::", stringify!(QueryStorageRequest));
    const FIELDS: &'static [crate::schema::declaration::Field] = &[];

    const MARKER: &'static [u8] = &crate::schema::declaration::encode::<
        { crate::schema::declaration::encoded_len(Self::NAME, Self::FIELDS) },
    >(Self::NAME, Self::FIELDS);
}

#[cfg(feature = "grpc")]
impl crate::__private::Protobuf for QueryStorageRequest {
    fn encode_to_vec(&self) -> Vec<u8> {
        Vec::new()
    }

    fn encode_into(&self, _: &mut Vec<u8>) {}
    fn encoded_len(&self) -> usize {
        0
    }

    fn decode(bytes: &[u8]) -> std::result::Result<Self, crate::MessageError> {
        <Self as crate::__private::Protobuf>::decode_input(crate::__private::DecodeInput::new(
            bytes,
        ))
    }

    fn decoding_memory(bytes: &[u8]) -> std::result::Result<usize, crate::MessageError> {
        <Self as crate::__private::Protobuf>::decoding_memory_input(
            crate::__private::DecodeInput::new(bytes),
        )
    }

    fn decoding_memory_input(
        input: crate::__private::DecodeInput<'_>,
    ) -> std::result::Result<usize, crate::MessageError> {
        input.fields(&mut |_| Ok(()))?;
        Ok(0)
    }

    fn decode_input(
        input: crate::__private::DecodeInput<'_>,
    ) -> std::result::Result<Self, crate::MessageError> {
        <Self as crate::__private::Protobuf>::decoding_memory_input(input)?;
        Ok(Self)
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
