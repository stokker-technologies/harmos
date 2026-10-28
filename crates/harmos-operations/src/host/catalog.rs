/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

#[cfg(feature = "runtime")]
use std::{num::NonZeroUsize, sync::Arc};

#[cfg(feature = "runtime")]
use serde::Serialize;

#[cfg(feature = "runtime")]
use crate::{Error, Operation, Response};
use harmos_domain::model::storage::codec::{Bytes, Code, Id, Index, Optional, Text, coded, product};
use harmos_domain::plugin::Grant;
use harmos_domain::{Identifier, MemorySize};
#[cfg(feature = "runtime")]
use harmos_sdk::operations::access::DiscoveryAccess;
#[cfg(feature = "runtime")]
use harmos_storage::{
    ErasedCatalog, IdentityKind, IdentitySelection, ReadLimits, StorageManager, StorageSchema, StoredIdentity, query::SchemaSelection,
};

product!(Catalog {
    kind: Code<CatalogKind> => 1,
    schema: Optional<Id> => 2,
    after: Optional<Text> => 3,
    rows: Optional<Index> => 4,
    bytes: Optional<Index> => 5,
});

product!(CatalogPage { entries: Bytes => 1, next: Optional<Text> => 2 });

coded!(CatalogKind { Operations, Schemas, Streams, Objects });

#[harmos::operation(
    harmos_path = crate,
    resources = [
        Option<Arc<StorageManager<ErasedCatalog>>>,
        DiscoveryAccess,
    ],
    codec = manual,
    id = "host.catalog",
    response = CatalogPage,
    policy = super::POLICY,
    visibility = Shared,
    grant = Catalog::required_grant,
    grpc(package = "harmos.host.v1", service = "Host", method = "Catalog"),
)]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Catalog {
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

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum CatalogKind {
    #[default]
    Operations,

    Schemas,
    Streams,
    Objects,
}

impl Catalog {
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

impl MemorySize for Catalog {
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

#[cfg(feature = "runtime")]
type Result<T> = std::result::Result<T, Error>;
#[cfg(feature = "runtime")]
impl Operation for Catalog {
    type Error = Error;

    async fn run(self, context: harmos_sdk::Context<'_, Self>) -> Result<Response<CatalogPage>> {
        let storage = context.resource::<Option<std::sync::Arc<harmos_storage::StorageManager<harmos_storage::ErasedCatalog>>>>();
        let access = context.resource::<harmos_sdk::operations::access::DiscoveryAccess>();

        let host = access;

        let page = match self.kind {
            CatalogKind::Operations => {
                let (rows, bytes) = self.limits()?;
                let (entries, next) = host.0.discover(self.after.as_deref(), rows, bytes).map_err(Error::Access)?;
                CatalogPage { entries, next }
            }
            CatalogKind::Schemas => {
                let schema = self.schema.clone();
                stored::<StorageSchema>(
                    storage.as_deref().as_ref().ok_or(Error::NotReady { domain: "catalog storage" })?,
                    &SchemaSelection { schema: None },
                    &self,
                    |entry| schema.as_ref().is_none_or(|schema| schema == &entry.key.id),
                )
                .await?
            }
            CatalogKind::Streams | CatalogKind::Objects => {
                let selection = IdentitySelection {
                    kind: if self.kind == CatalogKind::Streams { IdentityKind::Streams } else { IdentityKind::Objects },
                    schema: self.schema.clone(),
                };
                stored::<StoredIdentity>(
                    storage.as_deref().as_ref().ok_or(Error::NotReady { domain: "catalog storage" })?,
                    &selection,
                    &self,
                    |_| true,
                )
                .await?
            }
        };
        Ok(page.into())
    }
}

#[cfg(feature = "runtime")]
async fn stored<T>(
    storage: &StorageManager<ErasedCatalog>, selection: &T::Selection, request: &Catalog, keep: impl Fn(&T) -> bool,
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
        .map_err(|_| Error::InvalidInput {
            field: "catalog cursor",
            reason: "cursor must be a storage position",
        })?
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

impl Catalog {
    pub fn limits(&self) -> std::result::Result<(usize, usize), harmos_domain::MessageError> {
        let rows = self.rows.unwrap_or(64);
        let bytes = self.bytes.unwrap_or(64 * 1024);
        if rows == 0 || rows > 256 || !(2..=4 * 1024 * 1024).contains(&bytes) {
            return Err(harmos_domain::MessageError::borrowed("rows must be 1..=256 and bytes must be 2..=4194304"));
        }
        Ok((rows, bytes))
    }
}
