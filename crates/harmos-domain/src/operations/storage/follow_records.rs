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
// Third Party Imports
// Local Imports
use crate::model::storage::codec::{Nested, Optional, product};
use crate::model::storage::{RecordSelection, StorageCursor, StoredRecord};
use crate::schema::declaration::{Field, Kind};
use crate::schema::{Cardinality, Described};
use crate::{MemorySize, MessageError};

/* === Constants === */

/* === Macros === */

product!(FollowBoundaries {
    saved: Nested<StorageCursor> => 1,
    accepted: Nested<StorageCursor> => 2,
});

/* === Type Aliases === */

/* === Structs === */

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FollowRecordsRequest {
    pub after: Option<StorageCursor>,
    pub selection: RecordSelection,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FollowBoundaries {
    pub saved: StorageCursor,
    pub accepted: StorageCursor,
}

/* === Enums === */

#[derive(Clone, Debug, PartialEq)]
pub enum Followed {
    Boundaries(FollowBoundaries),
    Record(StoredRecord),
}

/* === Traits === */

/* === Implementations === */

product!(FollowRecordsRequest {
    after: Optional<Nested<StorageCursor>> => 1,
    selection: Nested<RecordSelection> => 2,
});

pub struct FollowRecords;

impl crate::operations::OperationDeclaration for FollowRecords {
    type Request = FollowRecordsRequest;
    type Response = StorageCursor;
    type Input = crate::operations::NoStream;
    type Output = crate::operations::Stream<Followed>;
    const ID: &'static str = "storage_follow_records";
    const POLICY: crate::operations::OperationPolicy = super::STREAM_POLICY;
    const VISIBILITY: crate::operations::Visibility = crate::operations::Visibility::Shared;
    const GRPC: Option<crate::operations::Grpc> = Some(crate::operations::Grpc::new("harmos.storage.v1", "Storage", "FollowRecords"));
    fn grant(request: &Self::Request) -> Option<crate::plugin::Grant> {
        Some(crate::plugin::Grant::of(Self::ID, request.selection.schema.id.as_str().to_owned()))
    }
}

impl crate::operations::RequestFor for FollowRecordsRequest {
    type Operation = FollowRecords;
}

impl MemorySize for FollowRecordsRequest {
    fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>()
            .saturating_add(self.after.memory_size().saturating_sub(std::mem::size_of_val(&self.after)))
            .saturating_add(self.selection.memory_size().saturating_sub(std::mem::size_of_val(&self.selection)))
    }
}

impl MemorySize for FollowBoundaries {
    fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>()
            .saturating_add(self.saved.memory_size().saturating_sub(std::mem::size_of::<StorageCursor>()))
            .saturating_add(self.accepted.memory_size().saturating_sub(std::mem::size_of::<StorageCursor>()))
    }
}

impl MemorySize for Followed {
    fn memory_size(&self) -> usize {
        let heap = match self {
            Self::Boundaries(value) => value.memory_size().saturating_sub(std::mem::size_of::<FollowBoundaries>()),
            Self::Record(value) => value.memory_size().saturating_sub(std::mem::size_of::<StoredRecord>()),
        };
        std::mem::size_of::<Self>().saturating_add(heap)
    }
}

impl Described for Followed {
    const NAME: &'static str = concat!(module_path!(), "::", stringify!(Followed));
    const FIELDS: &'static [Field] = &[
        Field {
            name: Cow::Borrowed("boundaries"),
            tag: 1,
            cardinality: Cardinality::Variant,
            kind: Kind::Message { definition: FollowBoundaries::DEFINITION },
        },
        Field {
            name: Cow::Borrowed("record"),
            tag: 2,
            cardinality: Cardinality::Variant,
            kind: Kind::Message { definition: StoredRecord::DEFINITION },
        },
    ];

    const MARKER: &'static [u8] = &crate::schema::declaration::encode::<
        { crate::schema::declaration::encoded_len(Self::NAME, Self::FIELDS) },
    >(Self::NAME, Self::FIELDS);
}

impl crate::__private::PostcardValue for Followed {
    fn write(&self, bytes: &mut Vec<u8>) {
        match self {
            Self::Boundaries(value) => {
                0u32.write(bytes);
                value.write(bytes);
            }
            Self::Record(value) => {
                1u32.write(bytes);
                value.write(bytes);
            }
        }
    }

    fn size(&self) -> usize {
        1usize.saturating_add(match self {
            Self::Boundaries(value) => value.size(),
            Self::Record(value) => value.size(),
        })
    }

    fn memory(input: &mut crate::__private::PostcardReader<'_>) -> std::result::Result<usize, MessageError> {
        input.nested(|input| {
            let heap = match input.value::<u32>()? {
                0 => <FollowBoundaries as crate::__private::PostcardValue>::memory(input)?
                    .saturating_sub(std::mem::size_of::<FollowBoundaries>()),
                1 => <StoredRecord as crate::__private::PostcardValue>::memory(input)?.saturating_sub(std::mem::size_of::<StoredRecord>()),
                _ => return Err(MessageError::borrowed("unknown follow output variant")),
            };
            crate::__private::decoding_add(std::mem::size_of::<Self>(), heap)
        })
    }

    fn read(input: &mut crate::__private::PostcardReader<'_>) -> std::result::Result<Self, MessageError> {
        input.nested(|input| match input.value::<u32>()? {
            0 => <FollowBoundaries as crate::__private::PostcardValue>::read(input).map(Self::Boundaries),
            1 => <StoredRecord as crate::__private::PostcardValue>::read(input).map(Self::Record),
            _ => Err(MessageError::borrowed("unknown follow output variant")),
        })
    }
}

impl Followed {
    pub fn wrapping_memory(record: usize) -> usize {
        std::mem::size_of::<Self>().saturating_add(record.saturating_sub(std::mem::size_of::<StoredRecord>()))
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
