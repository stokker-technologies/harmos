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
use std::any::TypeId;
use std::borrow::Cow;

use std::sync::Arc;

// Third Party Imports

// Local Imports

use crate::operations::{Context, Operation, StreamOutput};
use crate::operations::{Grpc, OperationDeclaration, OutputDeclaration, Visibility, WireOperation};
use crate::schema::declaration::{Field, Kind};
use crate::schema::{Cardinality, Described};

use crate::storage::StorageManager;
use crate::storage::codec::{Nested, product};
use crate::storage::follow::Records;
use crate::storage::{StorageCursor, StoredRecord};

use crate::{Error, Result};
use crate::{MemorySize, MessageError};
use harmos_contract::plugin::Grant;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */
pub struct Follow;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FollowBoundaries {
    pub saved: StorageCursor,
    pub accepted: StorageCursor,
}

pub(super) struct Handler<C> {
    storage: Arc<StorageManager<C>>,
}

/* === Enums === */
#[derive(Clone, Debug, PartialEq)]
pub enum Followed {
    Boundaries(FollowBoundaries),
    Record(StoredRecord),
}

/* === Traits === */

/* === Implementations === */
product!(FollowBoundaries {
    saved: Nested<StorageCursor> => 1,
    accepted: Nested<StorageCursor> => 2,
});

impl MemorySize for FollowBoundaries {
    fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>()
            .saturating_add(
                self.saved
                    .memory_size()
                    .saturating_sub(std::mem::size_of::<StorageCursor>()),
            )
            .saturating_add(
                self.accepted
                    .memory_size()
                    .saturating_sub(std::mem::size_of::<StorageCursor>()),
            )
    }
}

impl MemorySize for Followed {
    fn memory_size(&self) -> usize {
        let heap = match self {
            Self::Boundaries(value) => value
                .memory_size()
                .saturating_sub(std::mem::size_of::<FollowBoundaries>()),
            Self::Record(value) => value
                .memory_size()
                .saturating_sub(std::mem::size_of::<StoredRecord>()),
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
            kind: Kind::Message {
                definition: FollowBoundaries::DEFINITION,
            },
        },
        Field {
            name: Cow::Borrowed("record"),
            tag: 2,
            cardinality: Cardinality::Variant,
            kind: Kind::Message {
                definition: StoredRecord::DEFINITION,
            },
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

    fn memory(
        input: &mut crate::__private::PostcardReader<'_>,
    ) -> std::result::Result<usize, MessageError> {
        input.nested(|input| {
            let heap = match input.value::<u32>()? {
                0 => <FollowBoundaries as crate::__private::PostcardValue>::memory(input)?
                    .saturating_sub(std::mem::size_of::<FollowBoundaries>()),
                1 => <StoredRecord as crate::__private::PostcardValue>::memory(input)?
                    .saturating_sub(std::mem::size_of::<StoredRecord>()),
                _ => return Err(MessageError::borrowed("unknown follow output variant")),
            };
            crate::__private::decoding_add(std::mem::size_of::<Self>(), heap)
        })
    }

    fn read(
        input: &mut crate::__private::PostcardReader<'_>,
    ) -> std::result::Result<Self, MessageError> {
        input.nested(|input| match input.value::<u32>()? {
            0 => <FollowBoundaries as crate::__private::PostcardValue>::read(input)
                .map(Self::Boundaries),
            1 => <StoredRecord as crate::__private::PostcardValue>::read(input).map(Self::Record),
            _ => Err(MessageError::borrowed("unknown follow output variant")),
        })
    }
}

crate::with_protobuf! {
    impl crate::__private::Protobuf for Followed {
        fn encode_to_vec(&self) -> Vec<u8> {
            let mut bytes =
                Vec::with_capacity(<Self as crate::__private::Protobuf>::encoded_len(self));
            <Self as crate::__private::Protobuf>::encode_into(self, &mut bytes);
            bytes
        }
        fn encode_into(&self, bytes: &mut Vec<u8>) {
            use crate::storage::codec::Value;
            match self {
                Self::Boundaries(value) => Nested::<FollowBoundaries>::write(value, 1, bytes),
                Self::Record(value) => Nested::<StoredRecord>::write(value, 2, bytes),
            }
        }
        fn encoded_len(&self) -> usize {
            use crate::storage::codec::Value;
            match self {
                Self::Boundaries(value) => Nested::<FollowBoundaries>::size(value, 1),
                Self::Record(value) => Nested::<StoredRecord>::size(value, 2),
            }
        }
        fn decode(bytes: &[u8]) -> std::result::Result<Self, MessageError> {
            <Self as crate::__private::Protobuf>::decode_input(crate::__private::DecodeInput::new(bytes))
        }
        fn decoding_memory(bytes: &[u8]) -> std::result::Result<usize, MessageError> {
            <Self as crate::__private::Protobuf>::decoding_memory_input(crate::__private::DecodeInput::new(bytes))
        }
        fn decoding_memory_input(
            input: crate::__private::DecodeInput<'_>,
        ) -> std::result::Result<usize, MessageError> {
            let (tag, child) = input
                .variant(&[1, 2])?
                .ok_or_else(|| MessageError::borrowed("missing follow output variant"))?;
            let heap = match tag {
                1 => <FollowBoundaries as crate::__private::Protobuf>::decoding_memory_input(child)?
                    .saturating_sub(std::mem::size_of::<FollowBoundaries>()),
                _ => <StoredRecord as crate::__private::Protobuf>::decoding_memory_input(child)?
                    .saturating_sub(std::mem::size_of::<StoredRecord>()),
            };
            crate::__private::decoding_add(std::mem::size_of::<Self>(), heap)
        }
        fn decode_input(
            input: crate::__private::DecodeInput<'_>,
        ) -> std::result::Result<Self, MessageError> {
            let (tag, child) = input
                .variant(&[1, 2])?
                .ok_or_else(|| MessageError::borrowed("missing follow output variant"))?;
            match tag {
                1 => <FollowBoundaries as crate::__private::Protobuf>::decode_input(child)
                    .map(Self::Boundaries),
                _ => <StoredRecord as crate::__private::Protobuf>::decode_input(child)
                    .map(Self::Record),
            }
        }
    }
}

impl OperationDeclaration for Follow {
    type Request = Records;
    type Response = StorageCursor;
    const ID: &'static str = "storage.follow";
    const POLICY: crate::operations::OperationPolicy = super::STREAM_POLICY;
    const VISIBILITY: Visibility = Visibility::Shared;
    fn endpoint_types() -> (Option<TypeId>, Option<TypeId>) {
        (None, Some(TypeId::of::<Followed>()))
    }

    fn grant(request: &Records) -> Option<Grant> {
        Some(Grant::of(
            Self::ID,
            request.selection.schema.id.as_str().to_owned(),
        ))
    }
}

impl OutputDeclaration for Follow {
    type Item = Followed;
}

impl WireOperation for Follow {
    const VERSION: u32 = 1;
    const OUTPUT: Option<crate::schema::declaration::Definition> = Some(Followed::DEFINITION);
    const GRPC: Option<Grpc> = Some(super::grpc("Follow"));
    crate::with_protobuf! {
        fn visit_grpc<V: crate::operations::GrpcVisitor>(visitor: V) -> Option<V::Entry> {
            Some(visitor.download::<Self>())
        }
    }
}

impl<C: 'static> Operation for Handler<C> {
    type Declaration = Follow;
    type Resources = StorageManager<C>;
    type Error = Error;
    type Context<'a> = Context<'a>;
    type Endpoints = StreamOutput<Followed>;

    async fn initialize<'ctx>(
        storage: Arc<StorageManager<C>>,
        _: Self::Context<'ctx>,
    ) -> Result<Self> {
        Ok(Self { storage })
    }

    async fn run<'a>(
        &mut self,
        _: Self::Context<'a>,
        request: &mut Records,
        output: &mut Self::Endpoints,
    ) -> Result<crate::operations::Response<StorageCursor>> {
        crate::storage::query::Source::Records(request.selection.clone()).validate()?;
        let mut follower = self
            .storage
            .follow::<StoredRecord>(request.after.as_ref(), request.selection.clone())
            .await?;
        let first = Followed::Boundaries(FollowBoundaries {
            saved: follower.saved_boundary().clone(),
            accepted: follower.accepted_boundary().clone(),
        });
        output
            .send_with(first.memory_size(), || Ok(first.clone()))
            .await?;
        loop {
            let page = follower.next().await?;
            for entry in page.entries() {
                let wrapped = Followed::wrapping_memory(entry.memory_size());
                output
                    .send_with(wrapped, || Ok(Followed::Record(entry.clone())))
                    .await?;
            }
        }
    }
}

impl Followed {
    fn wrapping_memory(record: usize) -> usize {
        std::mem::size_of::<Self>()
            .saturating_add(record.saturating_sub(std::mem::size_of::<StoredRecord>()))
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
