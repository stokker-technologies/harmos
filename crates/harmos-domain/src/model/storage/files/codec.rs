/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

use std::borrow::Cow;
use std::num::NonZeroUsize;
use std::path::PathBuf;

use super::{FileMetadata, FileReference};

use crate::model::storage::StorageCursor;
use crate::model::storage::position::codec::{decode_bounded, encode_bounded};
use crate::schema::declaration::{Field, Kind};
use crate::schema::{Cardinality, Described};
use crate::{Error, MessageError, Result};

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

// Wire Values: Zero Producing, One Finished, Two Abandoned.
struct Phase;

// Native Paths May Be Non-Unicode; Transport Decoding Requires Portable UTF-8.
pub struct LocalPath;
pub struct Digest;

impl crate::model::storage::codec::Value for Digest {
    const NAME: &'static str = concat!(module_path!(), "::Digest");
    type Native = [u8; 32];
    const KIND: Kind = Kind::Bytes;
    fn postcard_write(value: &[u8; 32], bytes: &mut Vec<u8>) {
        bytes.push(32);
        bytes.extend_from_slice(value);
    }
    fn postcard_size(_: &[u8; 32]) -> usize {
        33
    }
    fn postcard_memory(input: &mut crate::__private::PostcardReader<'_>) -> std::result::Result<usize, MessageError> {
        Self::postcard_read(input)?;
        Ok(0)
    }
    fn postcard_read(input: &mut crate::__private::PostcardReader<'_>) -> std::result::Result<[u8; 32], MessageError> {
        if input.value::<usize>()? != 32 {
            return Err(MessageError::borrowed("SHA-256 requires exactly 32 bytes"));
        }
        input.take(32)?.try_into().map_err(|_| MessageError::borrowed("SHA-256 requires exactly 32 bytes"))
    }
}

impl crate::model::storage::codec::Value for LocalPath {
    const NAME: &'static str = concat!(module_path!(), "::LocalPath");
    type Native = PathBuf;
    const KIND: Kind = Kind::Bytes;

    fn postcard_write(value: &PathBuf, bytes: &mut Vec<u8>) {
        let path = value.as_os_str().as_encoded_bytes();
        crate::__private::PostcardValue::write(&(path.len() as u64), bytes);
        bytes.extend_from_slice(path);
    }

    fn postcard_size(value: &PathBuf) -> usize {
        let length = value.as_os_str().as_encoded_bytes().len();
        crate::__private::PostcardValue::size(&(length as u64)).saturating_add(length)
    }

    fn postcard_memory(input: &mut crate::__private::PostcardReader<'_>) -> std::result::Result<usize, MessageError> {
        <crate::model::storage::codec::Bytes as crate::model::storage::codec::Value>::postcard_memory(input)
    }

    fn postcard_read(input: &mut crate::__private::PostcardReader<'_>) -> std::result::Result<PathBuf, MessageError> {
        let bytes = <crate::model::storage::codec::Bytes as crate::model::storage::codec::Value>::postcard_read(input)?;
        let path = String::from_utf8(bytes).map_err(|_| MessageError::borrowed("transported local paths must be UTF-8"))?;
        Ok(PathBuf::from(path))
    }
}

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

    fn postcard_memory(input: &mut crate::__private::PostcardReader<'_>) -> std::result::Result<usize, MessageError> {
        Self::postcard_read(input)?;
        Ok(0)
    }

    fn postcard_read(input: &mut crate::__private::PostcardReader<'_>) -> std::result::Result<Self::Native, MessageError> {
        match input.value::<u64>()? {
            0 => Ok(super::FilePhase::Producing),
            1 => Ok(super::FilePhase::Finished),
            2 => Ok(super::FilePhase::Abandoned),
            _ => Err(MessageError::borrowed("invalid storage selector")),
        }
    }

    type Native = super::FilePhase;
    const KIND: Kind = Kind::U64;
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
            || self.metadata.expected_length.is_some_and(|length| length != self.length)
            || self.metadata.expected_sha256.is_some_and(|hash| hash != self.sha256)
        {
            return Err(MessageError::borrowed("file reference conflicts with its position or integrity requirements"));
        }
        Ok(())
    }
}

crate::postcard_product!(FileMetadata {
    id: crate::Identifier,
    scope: Option<crate::Identifier>,
    media_type: String,
    expected_length: Option<u64>,
    expected_sha256: Option<[u8; 32]>,
});

crate::postcard_product!(
    FileReference {
        position: StorageCursor,
        metadata: FileMetadata,
        length: u64,
        sha256: [u8; 32]
    },
    validate = FileReference::check_native
);

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
            kind: Kind::Message { definition: StorageCursor::DEFINITION },
            cardinality: Cardinality::Required,
        },
        Field {
            name: Cow::Borrowed("metadata"),
            tag: 2,
            kind: Kind::Message { definition: FileMetadata::DEFINITION },
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
