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
use harmos_domain::model::storage::codec::{Nested, Optional, product};
use harmos_domain::model::storage::{RecordSelection, StorageCursor, StoredRecord};
use harmos_domain::schema::declaration::{Field, Kind};
use harmos_domain::schema::{Cardinality, Described};
use harmos_domain::{MemorySize, MessageError};
#[cfg(feature = "runtime")]
use harmos_storage::{ErasedCatalog, StorageManager};

/* === Constants === */

/* === Macros === */

product!(FollowBoundaries {
    saved: Nested<StorageCursor> => 1,
    accepted: Nested<StorageCursor> => 2,
});

/* === Type Aliases === */

/* === Structs === */

#[harmos::operation(harmos_path = crate, resources = [StorageManager<ErasedCatalog>], codec = manual,
    id = "storage.follow_records",
    response = StorageCursor,
    policy = super::STREAM_POLICY,
    output = Followed,
    visibility = Public,
    grpc(package = "harmos.storage.v1", service =  "Storage", method =  "FollowRecords"),
    grant = Self::grant_for)]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FollowRecords {
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

product!(FollowRecords {
    after: Optional<Nested<StorageCursor>> => 1,
    selection: Nested<RecordSelection> => 2,
});

impl FollowRecords {
    fn grant_for(request: &FollowRecords) -> Option<harmos_domain::plugin::Grant> {
        Some(harmos_domain::plugin::Grant::of(
            <Self as harmos_domain::OperationDeclaration>::ID,
            request.selection.schema.id.as_str().to_owned(),
        ))
    }
}

impl MemorySize for FollowRecords {
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

    const MARKER: &'static [u8] = &harmos_domain::schema::declaration::encode::<
        { harmos_domain::schema::declaration::encoded_len(Self::NAME, Self::FIELDS) },
    >(Self::NAME, Self::FIELDS);
}

impl harmos_domain::__private::PostcardValue for Followed {
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

    fn memory(input: &mut harmos_domain::__private::PostcardReader<'_>) -> std::result::Result<usize, MessageError> {
        input.nested(|input| {
            let heap = match input.value::<u32>()? {
                0 => <FollowBoundaries as harmos_domain::__private::PostcardValue>::memory(input)?
                    .saturating_sub(std::mem::size_of::<FollowBoundaries>()),
                1 => <StoredRecord as harmos_domain::__private::PostcardValue>::memory(input)?
                    .saturating_sub(std::mem::size_of::<StoredRecord>()),
                _ => return Err(MessageError::borrowed("unknown follow output variant")),
            };
            harmos_domain::__private::decoding_add(std::mem::size_of::<Self>(), heap)
        })
    }

    fn read(input: &mut harmos_domain::__private::PostcardReader<'_>) -> std::result::Result<Self, MessageError> {
        input.nested(|input| match input.value::<u32>()? {
            0 => <FollowBoundaries as harmos_domain::__private::PostcardValue>::read(input).map(Self::Boundaries),
            1 => <StoredRecord as harmos_domain::__private::PostcardValue>::read(input).map(Self::Record),
            _ => Err(MessageError::borrowed("unknown follow output variant")),
        })
    }
}

impl Followed {
    pub fn wrapping_memory(record: usize) -> usize {
        std::mem::size_of::<Self>().saturating_add(record.saturating_sub(std::mem::size_of::<StoredRecord>()))
    }
}

#[cfg(feature = "runtime")]
mod execution {
    use super::*;

    // Standard Library Imports

    // Third Party Imports

    // Local Imports

    use harmos_sdk::operations::Operation;

    use harmos_storage::{StorageCursor, StoredRecord};

    use crate::MemorySize;
    use crate::{Error, Result};

    impl Operation for super::FollowRecords {
        type Error = Error;

        async fn run(self, context: harmos_sdk::Context<'_, Self>) -> Result<harmos_sdk::operations::Response<StorageCursor>> {
            let storage = context.resource::<harmos_storage::StorageManager<harmos_storage::ErasedCatalog>>();
            let output = context.streams();

            harmos_storage::query::Source::Records(self.selection.clone()).validate()?;
            let mut follower = storage.follow::<StoredRecord>(self.after.as_ref(), self.selection.clone()).await?;
            let first = Followed::Boundaries(FollowBoundaries {
                saved: follower.saved_boundary().clone(),
                accepted: follower.accepted_boundary().clone(),
            });
            output.send_with(first.memory_size(), || Ok(first.clone())).await?;
            loop {
                let page = follower.next().await?;
                for entry in page.entries() {
                    let wrapped = Followed::wrapping_memory(entry.memory_size());
                    output.send_with(wrapped, || Ok(Followed::Record(entry.clone()))).await?;
                }
            }
        }
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
