/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Imports === */

pub use harmos_domain::model::operations::publication::FollowingRow;

/* === Macros === */

harmos_domain::builtin_payload! { PublishRequest,
    stream: String => (1, Kind::String, Required),
    schema: u32 => (2, Kind::U32, Required),
    sequence: Option<u64> => (3, Kind::U64, Optional),
    published_unix_micros: Option<i64> => (4, Kind::I64, Optional),
    context: Option<String> => (5, Kind::String, Optional),
    payload: Vec<u8> => (6, Kind::Bytes, Required),
    following: Vec<FollowingRow> => (7, Kind::Message { definition: FollowingRow::DEFINITION }, Repeated),
}

/* === Structs === */

#[harmos::operation(harmos_path = crate, id = "host_publish", request = PublishRequest,
 response = (), policy = crate::host::POLICY, infer_request,
 visibility = harmos_domain::operations::Visibility::Shared)]
pub struct Publish;

/* === Implementations === */

#[cfg(feature = "runtime")]
impl crate::Operation for Publish {
    type Request = PublishRequest;
    type Response = ();
    type Error = crate::Error;
    type Endpoints = ();
    async fn initialize(_: crate::OperationContext) -> Result<Self, Self::Error> {
        Ok(Self)
    }
    async fn run(
        &mut self, context: crate::OperationContext, request: &mut PublishRequest, _: &mut (),
    ) -> Result<crate::Response<()>, Self::Error> {
        let access = context.dependency::<harmos_sdk::operations::publication::PublicationAccess>()?;
        let publication = harmos_sdk::operations::publication::Publication {
            stream: &request.stream,
            schema: request.schema,
            sequence: request.sequence,
            published_unix_micros: request.published_unix_micros,
            context: &mut request.context,
            payload: &mut request.payload,
            following: &mut request.following,
        };
        access.0.publish(publication).await.map_err(crate::Error::Access)?;
        Ok(().into())
    }
}
