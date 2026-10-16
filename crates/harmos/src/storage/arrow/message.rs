/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Storage Arrow Query Contract

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */
// Standard Library Imports
use std::borrow::Cow;
// Local Imports
use super::{ArrowBatch, ArrowSchema, ipc};
#[cfg(feature = "grpc")]
use crate::__private::{DecodeInput, Protobuf};
use crate::__private::{PostcardReader, PostcardValue};
use crate::MessageError;
use crate::schema::declaration::{Field, Kind};
use crate::schema::{Cardinality, Described};
#[cfg(feature = "grpc")]
use crate::storage::position::codec::{bytes_field, write_bytes};

/* === Constants === */

/* === Macros === */
macro_rules! message {
    ($ty:ty, $batch:literal) => {
        impl Described for $ty {
            const NAME: &'static str = concat!(module_path!(), "::", stringify!($ty));
            const FIELDS: &'static [Field] = &[Field {
                name: Cow::Borrowed("ipc"),
                tag: 1,
                kind: Kind::Bytes,
                cardinality: Cardinality::Required,
            }];
            const MARKER: &'static [u8] = &crate::schema::declaration::encode::<
                { crate::schema::declaration::encoded_len(Self::NAME, Self::FIELDS) },
            >(Self::NAME, Self::FIELDS);
        }
        impl PostcardValue for $ty {
            fn write(&self, bytes: &mut Vec<u8>) {
                PostcardValue::write(self.ipc.as_ref(), bytes);
            }
            fn size(&self) -> usize {
                PostcardValue::size(self.ipc.as_ref())
            }
            fn memory(input: &mut PostcardReader<'_>) -> Result<usize, MessageError> {
                let bytes: &[u8] = input.value()?;
                memory(bytes, $batch)
            }
            fn read(input: &mut PostcardReader<'_>) -> Result<Self, MessageError> {
                let bytes: &[u8] = input.value()?;
                Self::from_ipc(bytes).map_err(|_| MessageError::borrowed("invalid Arrow IPC value"))
            }
        }
        #[cfg(feature = "grpc")]
        impl Protobuf for $ty {
            fn encode_to_vec(&self) -> Vec<u8> {
                let mut bytes = Vec::with_capacity(Protobuf::encoded_len(self));
                self.encode_into(&mut bytes);
                bytes
            }
            fn encode_into(&self, bytes: &mut Vec<u8>) {
                write_bytes(1, &self.ipc, bytes);
            }
            fn encoded_len(&self) -> usize {
                crate::storage::codec::length_delimited_size(1, self.ipc.len())
            }
            fn decode(bytes: &[u8]) -> Result<Self, MessageError> {
                Self::decode_input(DecodeInput::new(bytes))
            }
            fn decoding_memory(bytes: &[u8]) -> Result<usize, MessageError> {
                Self::decoding_memory_input(DecodeInput::new(bytes))
            }
            fn decoding_memory_input(input: DecodeInput<'_>) -> Result<usize, MessageError> {
                let bytes = bytes_field(input, 1)?
                    .ok_or_else(|| MessageError::borrowed("missing Arrow IPC bytes"))?;
                memory(bytes, $batch)
            }
            fn decode_input(input: DecodeInput<'_>) -> Result<Self, MessageError> {
                Self::decoding_memory_input(input)?;
                let bytes = bytes_field(input, 1)?
                    .ok_or_else(|| MessageError::borrowed("missing Arrow IPC bytes"))?;
                Self::from_ipc(bytes).map_err(|_| MessageError::borrowed("invalid Arrow IPC value"))
            }
        }
    };
}

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */
message!(ArrowSchema, false);
message!(ArrowBatch, true);

/* === Public Functions === */

/* === Private Functions === */
fn memory(bytes: &[u8], batch: bool) -> Result<usize, MessageError> {
    ipc::preflight(bytes, batch)
        .and_then(|bound| {
            bound
                .checked_add(std::mem::size_of::<ArrowBatch>())
                .ok_or_else(|| super::invalid("Arrow message bound overflow"))
        })
        .map_err(|_| MessageError::borrowed("invalid or excessive Arrow IPC value"))
}

/* === Tests === */
