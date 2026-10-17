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
use std::borrow::Cow;
use std::num::NonZeroUsize;
// Third Party Imports

// Local Imports
use super::{FileMetadata, FileReference};
#[cfg(feature = "grpc")]
use crate::__private::DecodeInput;
#[cfg(feature = "grpc")]
use crate::__private::decoding_add;

use crate::model::storage::StorageCursor;
#[cfg(feature = "grpc")]
use crate::model::storage::position::codec::{
    bytes_field, field_len, identifier, text_field, uint_field, write_bytes, write_cursor,
    write_header,
};
use crate::model::storage::position::codec::{decode_bounded, encode_bounded};
use crate::schema::declaration::{Field, Kind};
use crate::schema::{Cardinality, Described};
use crate::{Error, MessageError, Result};

/* === Constants === */

/* === Macros === */
crate::model::storage::codec::product!(super::FileSession {
    history: crate::model::storage::codec::Nested<crate::model::storage::StoreHistory> => 1,
    id: crate::model::storage::codec::Id => 2,
    generation: crate::model::storage::codec::Unsigned => 3,
});

crate::model::storage::codec::product!(super::StagedFile {
    history: crate::model::storage::codec::Nested<crate::model::storage::StoreHistory> => 1,
    id: crate::model::storage::codec::Id => 2,
});
crate::model::storage::codec::product!(super::FileProgress {
    length: crate::model::storage::codec::Unsigned => 1,
    phase: Phase => 2,
});

/* === Type Aliases === */

/* === Structs === */
// Wire Values: Zero Producing, One Finished, Two Abandoned.
struct Phase;

/* === Enums === */

/* === Traits === */

/* === Implementations === */
impl crate::model::storage::codec::Value for Phase {
    const NAME: &'static str = concat!(module_path!(), "::", stringify!(Phase));
    fn postcard_write(value: &Self::Native, bytes: &mut Vec<u8>) {
        crate::__private::PostcardValue::write(
            &match value {
                super::FilePhase::Producing => 0u64,
                super::FilePhase::Finished => 1u64,
                super::FilePhase::Abandoned => 2u64,
            },
            bytes,
        );
    }

    fn postcard_size(_: &Self::Native) -> usize {
        1
    }

    fn postcard_memory(
        input: &mut crate::__private::PostcardReader<'_>,
    ) -> std::result::Result<usize, MessageError> {
        Self::postcard_read(input)?;
        Ok(0)
    }

    fn postcard_read(
        input: &mut crate::__private::PostcardReader<'_>,
    ) -> std::result::Result<Self::Native, MessageError> {
        match input.value::<u64>()? {
            0 => Ok(super::FilePhase::Producing),
            1 => Ok(super::FilePhase::Finished),
            2 => Ok(super::FilePhase::Abandoned),
            _ => Err(MessageError::borrowed("invalid storage selector")),
        }
    }

    type Native = super::FilePhase;
    const KIND: Kind = Kind::U64;
    #[cfg(feature = "grpc")]
    fn write(value: &Self::Native, tag: u32, bytes: &mut Vec<u8>) {
        crate::__private::prost::encoding::uint64::encode(tag, &(*value as u64), bytes);
    }

    #[cfg(feature = "grpc")]
    fn size(value: &Self::Native, tag: u32) -> usize {
        crate::__private::prost::encoding::uint64::encoded_len(tag, &(*value as u64))
    }

    #[cfg(feature = "grpc")]
    fn memory(input: DecodeInput<'_>, tag: u32) -> std::result::Result<usize, MessageError> {
        Self::read(input, tag)?;
        Ok(0)
    }

    #[cfg(feature = "grpc")]
    fn read(input: DecodeInput<'_>, tag: u32) -> std::result::Result<Self::Native, MessageError> {
        match uint_field(input, tag)?.unwrap_or(0) {
            0 => Ok(super::FilePhase::Producing),
            1 => Ok(super::FilePhase::Finished),
            2 => Ok(super::FilePhase::Abandoned),
            _ => Err(MessageError::borrowed("unknown file phase")),
        }
    }
}

impl FileMetadata {
    pub fn encode_bounded(&self, max_bytes: NonZeroUsize) -> Result<Vec<u8>> {
        encode_bounded(self, max_bytes)
    }

    pub fn decode_bounded(bytes: &[u8], max_bytes: NonZeroUsize) -> Result<Self> {
        decode_bounded(bytes, max_bytes)
    }
}

impl FileReference {
    pub fn encode_bounded(&self, max_bytes: NonZeroUsize) -> Result<Vec<u8>> {
        self.check_native().map_err(|_| Error::InvalidInput {
            field: "storage file codec",
            reason: "invalid file reference",
        })?;
        encode_bounded(self, max_bytes)
    }

    pub fn decode_bounded(bytes: &[u8], max_bytes: NonZeroUsize) -> Result<Self> {
        decode_bounded(bytes, max_bytes)
    }

    pub fn check_native(&self) -> std::result::Result<(), MessageError> {
        if self.position.sequence == 0
            || self
                .metadata
                .expected_length
                .is_some_and(|length| length != self.length)
            || self
                .metadata
                .expected_sha256
                .is_some_and(|hash| hash != self.sha256)
        {
            return Err(MessageError::borrowed(
                "file reference conflicts with its position or integrity requirements",
            ));
        }
        Ok(())
    }
}

#[cfg(feature = "grpc")]
impl crate::__private::Protobuf for FileMetadata {
    fn encode_to_vec(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(<Self as crate::__private::Protobuf>::encoded_len(self));
        write_metadata(self, &mut bytes);
        bytes
    }

    fn encoded_len(&self) -> usize {
        use crate::__private::prost::encoding::encoded_len_varint;
        let mut length =
            field_len(self.id.as_str().len()).saturating_add(field_len(self.media_type.len()));
        if let Some(scope) = &self.scope {
            length = length.saturating_add(field_len(scope.as_str().len()));
        }
        if let Some(expected) = self.expected_length {
            length = length.saturating_add(1 + encoded_len_varint(expected));
        }
        if self.expected_sha256.is_some() {
            length = length.saturating_add(field_len(32));
        }
        length
    }

    fn decode(bytes: &[u8]) -> std::result::Result<Self, MessageError> {
        <Self as crate::__private::Protobuf>::decode_input(DecodeInput::new(bytes))
    }

    fn decoding_memory(bytes: &[u8]) -> std::result::Result<usize, MessageError> {
        <Self as crate::__private::Protobuf>::decoding_memory_input(DecodeInput::new(bytes))
    }

    fn decoding_memory_input(input: DecodeInput<'_>) -> std::result::Result<usize, MessageError> {
        let id = text_field(input, 1)?.unwrap_or("");
        let scope = text_field(input, 2)?;
        let media_type =
            text_field(input, 3)?.ok_or_else(|| MessageError::borrowed("missing media type"))?;
        uint_field(input, 4)?;
        bytes_field(input, 5)?.map(hash).transpose()?;
        let owned = decoding_add(std::mem::size_of::<Self>(), id.len())?;
        let owned = decoding_add(owned, scope.map_or(0, str::len))?;
        decoding_add(owned, media_type.len())
    }

    fn decode_input(input: DecodeInput<'_>) -> std::result::Result<Self, MessageError> {
        let id = identifier(text_field(input, 1)?.unwrap_or(""))?;
        let scope = text_field(input, 2)?.map(identifier).transpose()?;
        let media_type =
            text_field(input, 3)?.ok_or_else(|| MessageError::borrowed("missing media type"))?;
        let expected_length = uint_field(input, 4)?;
        let expected_sha256 = bytes_field(input, 5)?.map(hash).transpose()?;
        Ok(Self {
            id,
            scope,
            media_type: Box::<str>::from(media_type).into_string(),
            expected_length,
            expected_sha256,
        })
    }
}

#[cfg(feature = "grpc")]
impl crate::__private::Protobuf for FileReference {
    fn encode_to_vec(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(<Self as crate::__private::Protobuf>::encoded_len(self));
        write_header(
            1,
            crate::__private::Protobuf::encoded_len(&self.position),
            &mut bytes,
        );
        write_cursor(&self.position, &mut bytes);
        write_header(
            2,
            crate::__private::Protobuf::encoded_len(&self.metadata),
            &mut bytes,
        );
        write_metadata(&self.metadata, &mut bytes);
        crate::__private::prost::encoding::uint64::encode(3, &self.length, &mut bytes);
        write_bytes(4, &self.sha256, &mut bytes);
        bytes
    }

    fn encoded_len(&self) -> usize {
        use crate::__private::prost::encoding::encoded_len_varint;
        field_len(crate::__private::Protobuf::encoded_len(&self.position))
            .saturating_add(field_len(crate::__private::Protobuf::encoded_len(
                &self.metadata,
            )))
            .saturating_add(1 + encoded_len_varint(self.length))
            .saturating_add(field_len(32))
    }

    fn decode(bytes: &[u8]) -> std::result::Result<Self, MessageError> {
        <Self as crate::__private::Protobuf>::decode_input(DecodeInput::new(bytes))
    }

    fn decoding_memory(bytes: &[u8]) -> std::result::Result<usize, MessageError> {
        <Self as crate::__private::Protobuf>::decoding_memory_input(DecodeInput::new(bytes))
    }

    fn decoding_memory_input(input: DecodeInput<'_>) -> std::result::Result<usize, MessageError> {
        bytes_field(input, 1)?.ok_or_else(|| MessageError::borrowed("missing file position"))?;
        bytes_field(input, 2)?.ok_or_else(|| MessageError::borrowed("missing file metadata"))?;
        uint_field(input, 3)?.ok_or_else(|| MessageError::borrowed("missing file length"))?;
        hash(bytes_field(input, 4)?.ok_or_else(|| MessageError::borrowed("missing file sha256"))?)?;
        let position = StorageCursor::decoding_memory_input(input.field(1)?)?;
        let metadata = FileMetadata::decoding_memory_input(input.field(2)?)?;
        decoding_add(
            decoding_add(std::mem::size_of::<Self>(), position)?,
            metadata,
        )
    }

    fn decode_input(input: DecodeInput<'_>) -> std::result::Result<Self, MessageError> {
        bytes_field(input, 1)?.ok_or_else(|| MessageError::borrowed("missing file position"))?;
        bytes_field(input, 2)?.ok_or_else(|| MessageError::borrowed("missing file metadata"))?;
        let file = Self {
            position: StorageCursor::decode_input(input.field(1)?)?,
            metadata: FileMetadata::decode_input(input.field(2)?)?,
            length: uint_field(input, 3)?
                .ok_or_else(|| MessageError::borrowed("missing file length"))?,
            sha256: hash(
                bytes_field(input, 4)?
                    .ok_or_else(|| MessageError::borrowed("missing file sha256"))?,
            )?,
        };
        file.check_native()?;
        Ok(file)
    }
}

crate::postcard_product!(FileMetadata {
    id: crate::Identifier,
    scope: Option<crate::Identifier>,
    media_type: String,
    expected_length: Option<u64>,
    expected_sha256: Option<[u8; 32]>,
});

crate::postcard_product!(FileReference {
    position: StorageCursor,
    metadata: FileMetadata,
    length: u64,
    sha256: [u8; 32],
}, validate = FileReference::check_native);

impl Described for FileMetadata {
    const NAME: &'static str = concat!(module_path!(), "::", stringify!(FileMetadata));
    const FIELDS: &'static [Field] = &[
        Field {
            name: Cow::Borrowed("id"),
            tag: 1,
            kind: Kind::String,
            cardinality: Cardinality::Required,
        },
        Field {
            name: Cow::Borrowed("scope"),
            tag: 2,
            kind: Kind::String,
            cardinality: Cardinality::Optional,
        },
        Field {
            name: Cow::Borrowed("media_type"),
            tag: 3,
            kind: Kind::String,
            cardinality: Cardinality::Required,
        },
        Field {
            name: Cow::Borrowed("expected_length"),
            tag: 4,
            kind: Kind::U64,
            cardinality: Cardinality::Optional,
        },
        Field {
            name: Cow::Borrowed("expected_sha256"),
            tag: 5,
            kind: Kind::Bytes,
            cardinality: Cardinality::Optional,
        },
    ];

    const MARKER: &'static [u8] = &crate::schema::declaration::encode::<
        { crate::schema::declaration::encoded_len(Self::NAME, Self::FIELDS) },
    >(Self::NAME, Self::FIELDS);
}

impl Described for FileReference {
    const NAME: &'static str = concat!(module_path!(), "::", stringify!(FileReference));
    const FIELDS: &'static [Field] = &[
        Field {
            name: Cow::Borrowed("position"),
            tag: 1,
            kind: Kind::Message {
                definition: StorageCursor::DEFINITION,
            },
            cardinality: Cardinality::Required,
        },
        Field {
            name: Cow::Borrowed("metadata"),
            tag: 2,
            kind: Kind::Message {
                definition: FileMetadata::DEFINITION,
            },
            cardinality: Cardinality::Required,
        },
        Field {
            name: Cow::Borrowed("length"),
            tag: 3,
            kind: Kind::U64,
            cardinality: Cardinality::Required,
        },
        Field {
            name: Cow::Borrowed("sha256"),
            tag: 4,
            kind: Kind::Bytes,
            cardinality: Cardinality::Required,
        },
    ];

    const MARKER: &'static [u8] = &crate::schema::declaration::encode::<
        { crate::schema::declaration::encoded_len(Self::NAME, Self::FIELDS) },
    >(Self::NAME, Self::FIELDS);
}

/* === Public Functions === */

/* === Private Functions === */
#[cfg(feature = "grpc")]
fn write_metadata(metadata: &FileMetadata, bytes: &mut Vec<u8>) {
    write_bytes(1, metadata.id.as_str().as_bytes(), bytes);
    if let Some(scope) = &metadata.scope {
        write_bytes(2, scope.as_str().as_bytes(), bytes);
    }
    write_bytes(3, metadata.media_type.as_bytes(), bytes);
    if let Some(length) = metadata.expected_length {
        crate::__private::prost::encoding::uint64::encode(4, &length, bytes);
    }
    if let Some(hash) = &metadata.expected_sha256 {
        write_bytes(5, hash, bytes);
    }
}

#[cfg(feature = "grpc")]
fn hash(bytes: &[u8]) -> std::result::Result<[u8; 32], MessageError> {
    bytes
        .try_into()
        .map_err(|_| MessageError::borrowed("file sha256 must contain exactly 32 bytes"))
}

/* === Tests === */
