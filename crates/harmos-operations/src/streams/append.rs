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
use crate::execution::operations::{ExecutionContext, ProtobufOperation, Resource, ServingError, Visibility};

/* === Structs === */

pub struct Append;

/* === Implementations === */

impl ProtobufOperation for Append {
    type Request = harmos_protocol::context::SamplesAppendRequest;
    type Response = harmos_protocol::context::SamplesAppendResponse;
    type Endpoints = ();
    const ID: &'static str = "harmos.context.v1.Samples/Append";
    const POLICY: crate::OperationPolicy = crate::host::POLICY;
    const VISIBILITY: Visibility = Visibility::Public;
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
        Ok(harmos_protocol::context::SamplesAppendResponse {})
    }
}
