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

// Third Party Imports

// Local Imports

/* === Constants === */

/* === Macros === */

harmos_domain::builtin_payload! {
    #[harmos_operations::execution::operation(harmos_path = harmos_operations::execution, id = "host.publish", policy = harmos_operations::host::POLICY,
        codec = manual, visibility = Public, )]
    Publish,
    stream: String => (1, Kind::String, Required),
    sequence: Option<u64> => (3, Kind::U64, Optional),
    published_unix_micros: Option<i64> => (4, Kind::I64, Optional),
    context: Option<String> => (5, Kind::String, Optional),
    payload: Vec<u8> => (6, Kind::Bytes, Required),
    following: Vec<FollowingRow> => (7, Kind::Message { definition: FollowingRow::DEFINITION }, Repeated),
}

harmos_domain::builtin_payload! { FollowingRow,
    payload: Vec<u8> => (1, Kind::Bytes, Required),
    published_delta_micros: Option<i64> => (2, Kind::I64, Optional),
}

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

// Publishes one row on a declared stream through the host's `host.publish` operation.
pub fn publish_on<H, S>(host: &H, row: &S::Row) -> impl std::future::Future<Output = std::result::Result<(), H::Error>> + Send
where
    H: harmos_operations::execution::operations::OperationHost,
    S: harmos_domain::StreamSpec,
    S::Row: harmos_domain::Message,
{
    use harmos_domain::Message;
    let published = host.resolve::<Publish, ()>(
        Publish {
            stream: S::ID.to_owned(),
            sequence: None,
            published_unix_micros: None,
            context: None,
            payload: row.encode(),
            following: Vec::new(),
        },
        None,
        (),
    );
    async move {
        published.await?;
        Ok(())
    }
}

static DECLARATIONS: [harmos_domain::catalog::Node<harmos_domain::operations::WireDeclaration>; 1] =
    [harmos_domain::catalog::Node::Leaf(harmos_domain::operations::WireDeclaration::of::<Publish>())];
pub struct Streams;
impl harmos_operations::operations::OperationCatalog for Streams {
    const CATALOG: harmos_operations::operations::Catalog =
        harmos_operations::operations::Catalog::of::<Self>("harmos.guest.streams", &DECLARATIONS, &[]);
    fn batch() -> harmos_operations::execution::__private::RegistrationBatch {
        harmos_operations::execution::__private::RegistrationBatch::new()
    }
}
impl Streams {
    pub const CATALOG: harmos_operations::operations::Catalog = <Self as harmos_operations::operations::OperationCatalog>::CATALOG;
}
