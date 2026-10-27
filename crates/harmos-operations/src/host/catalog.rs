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
use harmos_domain::model::storage::codec::{Bytes, Code, Id, Index, Optional, Text, coded, product};
use harmos_domain::plugin::Grant;
use harmos_domain::{Identifier, MemorySize};

/* === Constants === */

/* === Macros === */

product!(CatalogRequest {
    kind: Code<CatalogKind> => 1,
    schema: Optional<Id> => 2,
    after: Optional<Text> => 3,
    rows: Optional<Index> => 4,
    bytes: Optional<Index> => 5,
});

product!(CatalogPage { entries: Bytes => 1, next: Optional<Text> => 2 });

coded!(CatalogKind { Operations, Schemas, Streams, Objects });

/* === Type Aliases === */

/* === Structs === */

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CatalogRequest {
    pub kind: CatalogKind,
    pub schema: Option<Identifier>,
    pub after: Option<String>,
    pub rows: Option<usize>,
    pub bytes: Option<usize>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CatalogPage {
    pub entries: Vec<u8>,
    pub next: Option<String>,
}

/* === Enums === */

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum CatalogKind {
    #[default]
    Operations,

    Schemas,
    Streams,
    Objects,
}

/* === Traits === */

/* === Implementations === */

impl CatalogRequest {
    // Stored Data Follows The Storage Read Grant Grammar; Operation Discovery
    // Keeps Its Per-Contract Conditional Access.
    pub fn required_grant(&self) -> Option<Grant> {
        let (family, all) = match self.kind {
            CatalogKind::Operations => return None,
            CatalogKind::Schemas => ("schemas", "schemas:*"),
            CatalogKind::Streams => ("streams", "streams:*"),
            CatalogKind::Objects => ("objects", "objects:*"),
        };
        Some(Grant::of(
            <Catalog as harmos_domain::operations::OperationDeclaration>::ID,
            self.schema.as_ref().map_or_else(|| all.to_owned(), |schema| format!("{family}:schema:{schema}")),
        ))
    }
}

impl MemorySize for CatalogRequest {
    fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>()
            .saturating_add(self.schema.as_ref().map_or(0, |schema| schema.as_str().len()))
            .saturating_add(self.after.as_ref().map_or(0, String::capacity))
    }
}

impl MemorySize for CatalogPage {
    fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>().saturating_add(self.entries.capacity()).saturating_add(self.next.as_ref().map_or(0, String::capacity))
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */

#[harmos::operation(harmos_path = crate, id = "host_catalog", request = CatalogRequest,
 response = CatalogPage, policy = super::POLICY, infer_request,
 visibility = harmos_domain::operations::Visibility::Shared, grant = CatalogRequest::required_grant,
 grpc(package = "harmos.host.v1", service = "Host", method = "Catalog"))]
pub struct Catalog {
    #[cfg(feature = "runtime")]
    storage: std::sync::Arc<Option<std::sync::Arc<harmos_storage::StorageManager<harmos_storage::ErasedCatalog>>>>,
}

#[cfg(feature = "runtime")]
use {
    crate::{Error, Operation, Response},
    harmos_storage::{
        ErasedCatalog, IdentityKind, IdentitySelection, ReadLimits, StorageManager, StorageSchema, StoredIdentity, query::SchemaSelection,
    },
    serde::Serialize,
    std::{num::NonZeroUsize, sync::Arc},
};
#[cfg(feature = "runtime")]
type Result<T> = std::result::Result<T, Error>;
#[cfg(feature = "runtime")]
impl Operation for Catalog {
    type Request = CatalogRequest;
    type Response = CatalogPage;
    type Error = Error;
    type Endpoints = ();

    async fn initialize(context: harmos_sdk::OperationContext) -> Result<Self> {
        let storage = context.dependency::<Option<Arc<StorageManager<ErasedCatalog>>>>()?;
        Ok(Self { storage })
    }

    async fn run(
        &mut self, context: harmos_sdk::OperationContext, request: &mut CatalogRequest, _: &mut Self::Endpoints,
    ) -> Result<Response<CatalogPage>> {
        let host = context.dependency::<harmos_sdk::operations::access::DiscoveryAccess>()?;

        let page = match request.kind {
            CatalogKind::Operations => {
                let (rows, bytes) = request.limits()?;
                let (entries, next) = host.0.discover(request.after.as_deref(), rows, bytes).map_err(Error::Access)?;
                CatalogPage { entries, next }
            }
            CatalogKind::Schemas => {
                let schema = request.schema.clone();
                stored::<StorageSchema>(self.storage()?, &SchemaSelection { schema: None }, request, |entry| {
                    schema.as_ref().is_none_or(|schema| schema == &entry.key.id)
                })
                .await?
            }
            CatalogKind::Streams | CatalogKind::Objects => {
                let selection = IdentitySelection {
                    kind: if request.kind == CatalogKind::Streams { IdentityKind::Streams } else { IdentityKind::Objects },
                    schema: request.schema.clone(),
                };
                stored::<StoredIdentity>(self.storage()?, &selection, request, |_| true).await?
            }
        };
        Ok(page.into())
    }
}

#[cfg(feature = "runtime")]
impl Catalog {
    fn storage(&self) -> Result<&StorageManager<ErasedCatalog>> {
        self.storage.as_deref().ok_or(Error::NotReady { domain: "catalog storage" })
    }
}

#[cfg(feature = "runtime")]
async fn stored<T>(
    storage: &StorageManager<ErasedCatalog>, selection: &T::Selection, request: &CatalogRequest, keep: impl Fn(&T) -> bool,
) -> Result<CatalogPage>
where
    T: harmos_storage::follow::Entry,
{
    let (rows, bytes) = request.limits()?;
    let after = request
        .after
        .as_deref()
        .map(str::parse::<u64>)
        .transpose()
        .map_err(|_| Error::InvalidInput { field: "catalog cursor", reason: "cursor must be a storage position" })?
        .unwrap_or(0);
    let limits = ReadLimits {
        rows: NonZeroUsize::new(rows).expect("validated catalog rows"),
        bytes: NonZeroUsize::new(bytes).expect("validated catalog bytes"),
    };
    let page = storage.page::<T>(selection, after, limits).await?;
    let mut entries = vec![b'['];
    let mut next = None;
    let mut count = 0usize;
    for entry in page.entries() {
        let position = entry.position().sequence;
        if keep(entry) {
            let start = entries.len();
            if count > 0 {
                entries.push(b',');
            }
            if write(&mut entries, entry).is_err() || entries.len() + 1 > bytes {
                entries.truncate(start);
                if next.is_none() {
                    return Err(Error::ResourceExhausted { resource: "catalog entry bytes" });
                }
                break;
            }
            count += 1;
        }
        next = Some(position.to_string());
    }
    entries.push(b']');
    Ok(CatalogPage { entries, next })
}

#[cfg(feature = "runtime")]
fn write(bytes: &mut Vec<u8>, entry: &impl Serialize) -> std::result::Result<(), serde_json::Error> {
    serde_json::to_writer(bytes, entry)
}

impl CatalogRequest {
    pub fn limits(&self) -> std::result::Result<(usize, usize), harmos_domain::MessageError> {
        let rows = self.rows.unwrap_or(64);
        let bytes = self.bytes.unwrap_or(64 * 1024);
        if rows == 0 || rows > 256 || !(2..=4 * 1024 * 1024).contains(&bytes) {
            return Err(harmos_domain::MessageError::borrowed("rows must be 1..=256 and bytes must be 2..=4194304"));
        }
        Ok((rows, bytes))
    }
}
