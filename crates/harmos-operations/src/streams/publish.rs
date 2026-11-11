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

pub struct Publish;

/* === Implementations === */

impl ProtobufOperation for Publish {
    type Request = harmos_protocol::host::Publish;
    type Response = harmos_protocol::host::Published;
    type Endpoints = ();
    const ID: &'static str = "host.publish";
    const POLICY: crate::OperationPolicy = crate::host::POLICY;
    const VISIBILITY: Visibility = Visibility::Public;
    const RESOURCES: &'static [Resource] = &[Resource::of::<PublicationAccess>()];

    async fn run(context: ExecutionContext, mut request: Self::Request, _: &()) -> Result<Self::Response, ServingError> {
        let access = context.resource::<PublicationAccess>().map_err(|_| ServingError::Catalog)?;
        let publication = crate::execution::operations::publication::Publication {
            stream: &request.stream,
            sequence: request.sequence,
            published_unix_micros: request.published_unix_micros,
            context: &mut request.context,
            payload: &mut request.payload,
            following: &mut request.following,
        };
        access
            .0
            .publish(context.call.clone(), publication)
            .await
            .map_err(|error| ServingError::Native(std::sync::Arc::new(crate::Error::Access(error))))?;
        Ok(harmos_protocol::host::Published {})
    }
}
