/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Imports === */

use crate::execution::operations::publication::PublicationAccess;
use crate::execution::operations::{ExecutionContext, NativeOperation, Resource, ServingError};

/* === Structs === */

pub struct Publish;

/* === Implementations === */

impl NativeOperation for Publish {
    type Request = super::PublishRequest;
    type Response = ();
    type Endpoints = ();
    const ID: &'static str = "streams.publish";
    const POLICY: crate::OperationPolicy = crate::host::POLICY;
    const RESOURCES: &'static [Resource] = &[Resource::of::<PublicationAccess>()];

    async fn run(context: ExecutionContext, mut request: Self::Request, _: &()) -> Result<Self::Response, ServingError> {
        let access = context.resource::<PublicationAccess>().map_err(|_| ServingError::Catalog)?;
        let published_unix_micros = request
            .published_at
            .map(|stamp| {
                if !(-62_135_596_800..=253_402_300_799).contains(&stamp.seconds) || !(0..1_000_000_000).contains(&stamp.nanos) {
                    return Err(ServingError::Codec(crate::MessageError::borrowed("Append requires a valid published_at Timestamp")));
                }
                Ok(stamp.seconds * 1_000_000 + i64::from(stamp.nanos) / 1_000)
            })
            .transpose()?;
        let publication = crate::execution::operations::publication::Publication {
            stream: &request.stream,
            sequence: request.sequence,
            published_unix_micros,
            context: &mut request.context,
            payload: &mut request.payload,
            following: &mut request.following,
        };
        access
            .0
            .publish(context.call.clone(), publication)
            .await
            .map_err(|error| ServingError::Native(std::sync::Arc::new(crate::Error::Access(error))))?;
        Ok(())
    }
}
