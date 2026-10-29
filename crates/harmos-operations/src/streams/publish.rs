/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

pub use harmos_domain::model::operations::publication::FollowingRow;

#[cfg(feature = "runtime")]
use harmos_sdk::operations::publication::PublicationAccess;

harmos_domain::builtin_payload! {
    #[harmos::operation(harmos_path = crate, id = "host.publish", policy = crate::host::POLICY,
        codec = manual, visibility = Public, resources = [PublicationAccess])]
    Publish,
    stream: String => (1, Kind::String, Required),
    schema: u32 => (2, Kind::U32, Required),
    sequence: Option<u64> => (3, Kind::U64, Optional),
    published_unix_micros: Option<i64> => (4, Kind::I64, Optional),
    context: Option<String> => (5, Kind::String, Optional),
    payload: Vec<u8> => (6, Kind::Bytes, Required),
    following: Vec<FollowingRow> => (7, Kind::Message { definition: FollowingRow::DEFINITION }, Repeated),
}

#[cfg(feature = "runtime")]
impl crate::Operation for Publish {
    type Error = crate::Error;

    async fn run(mut self, context: harmos_sdk::Context<'_, Self>) -> Result<crate::Response<()>, Self::Error> {
        let access = context.resource::<harmos_sdk::operations::publication::PublicationAccess>();

        let publication = harmos_sdk::operations::publication::Publication {
            stream: &self.stream,
            schema: self.schema,
            sequence: self.sequence,
            published_unix_micros: self.published_unix_micros,
            context: &mut self.context,
            payload: &mut self.payload,
            following: &mut self.following,
        };
        access.0.publish(context.call().clone(), publication).await.map_err(crate::Error::Access)?;
        Ok(().into())
    }
}
