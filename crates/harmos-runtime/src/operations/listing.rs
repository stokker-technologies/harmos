/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// The Paged Catalog Read: Live Operation Contracts From The Manager, And The
// Schemas, Streams And Objects Storage Holds Durably From Its Saved Frontier.

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::num::NonZeroUsize;
use std::sync::Arc;

// Third Party Imports
use serde::Serialize;

// Local Imports
use super::Operation;
use super::host::{Catalog, CatalogKind, CatalogPage, CatalogRequest};
use crate::storage::query::SchemaSelection;
use crate::storage::{IdentityKind, IdentitySelection, ReadLimits, StorageManager, StorageSchema, StoredIdentity};
use crate::{Error, Result};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

pub struct Listing<C> {
    storage: Arc<Option<Arc<StorageManager<C>>>>,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl<C: 'static> Operation for Listing<C> {
    type Request = CatalogRequest;
    type Response = CatalogPage;
    type Error = Error;
    type Endpoints = ();

    async fn initialize(context: harmos_sdk::OperationContext) -> Result<Self> {
        let storage = context.dependency::<Option<Arc<StorageManager<C>>>>()?;
        Ok(Self { storage })
    }

    async fn run(
        &mut self, context: harmos_sdk::OperationContext, request: &mut CatalogRequest, _: &mut Self::Endpoints,
    ) -> Result<super::Response<CatalogPage>> {
        let host = context.dependency::<super::HostContext>()?;
        let context = host.borrow();
        let page = match request.kind {
            CatalogKind::Operations => context.operations.discover(&context.source, request)?,
            CatalogKind::Schemas => {
                let schema = request.schema.clone();
                stored::<C, StorageSchema>(self.storage()?, &SchemaSelection { schema: None }, request, |entry| {
                    schema.as_ref().is_none_or(|schema| schema == &entry.key.id)
                })
                .await?
            }
            CatalogKind::Streams | CatalogKind::Objects => {
                let selection = IdentitySelection {
                    kind: if request.kind == CatalogKind::Streams { IdentityKind::Streams } else { IdentityKind::Objects },
                    schema: request.schema.clone(),
                };
                stored::<C, StoredIdentity>(self.storage()?, &selection, request, |_| true).await?
            }
        };
        Ok(page.into())
    }
}

impl<C> Listing<C> {
    fn storage(&self) -> Result<&StorageManager<C>> {
        self.storage.as_deref().ok_or(Error::NotReady { domain: "catalog storage" })
    }
}

/* === Public Functions === */

pub fn registration<C: 'static>(storage: Option<Arc<StorageManager<C>>>) -> Result<harmos_sdk::RegistrationBatch> {
    let mut dependencies = harmos_sdk::Dependencies::new();
    dependencies.insert(Arc::new(storage))?;
    Ok(harmos_sdk::RegistrationBatch::new(dependencies).implementation::<Catalog, Listing<C>>())
}
/* === Private Functions === */

// One Page Of Durable Entries After The Request's Storage Position. Pages Order
// By Storage Position, So A Nonempty Page Always Continues And An Empty Page
// Ends The Catalog.
async fn stored<C, T>(
    storage: &StorageManager<C>, selection: &T::Selection, request: &CatalogRequest, keep: impl Fn(&T) -> bool,
) -> Result<CatalogPage>
where
    T: crate::storage::follow::Entry,
{
    let (rows, bytes) = super::manager::limits(request)?;
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

fn write(bytes: &mut Vec<u8>, entry: &impl Serialize) -> std::result::Result<(), serde_json::Error> {
    serde_json::to_writer(bytes, entry)
}

/* === Tests === */
