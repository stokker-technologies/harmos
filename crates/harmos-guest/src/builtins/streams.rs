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
    #[derive(harmos_operations::execution::Request)]
#[harmos_path(harmos_operations::execution)]
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
    [harmos_domain::catalog::Node::Leaf(PUBLISH_OPERATION.wire())];
pub struct Streams;

pub static PUBLISH_DESCRIPTION: harmos_operations::execution::operations::WireDeclaration =
    harmos_operations::execution::operations::WireDeclaration::request::<Publish>("host.publish", harmos_operations::host::POLICY)
        .visibility(harmos_operations::execution::operations::Visibility::Public);

pub const PUBLISH_OPERATION: harmos_operations::execution::operations::OperationSpec<Publish> =
    harmos_operations::execution::operations::OperationSpec::<Publish>::new(&PUBLISH_DESCRIPTION);
impl harmos_operations::execution::operations::OperationCatalog for Streams {
    const CATALOG: harmos_operations::execution::operations::Catalog =
        harmos_operations::execution::operations::Catalog::of::<Self>("harmos.guest.streams", &DECLARATIONS, &[]);
    fn batch() -> harmos_operations::execution::__private::RegistrationBatch {
        harmos_operations::execution::__private::RegistrationBatch::new()
    }
}
impl Streams {
    pub const CATALOG: harmos_operations::execution::operations::Catalog =
        <Self as harmos_operations::execution::operations::OperationCatalog>::CATALOG;
}
