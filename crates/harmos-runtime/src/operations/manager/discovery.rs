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
use std::io::{self, Write};
use std::ops::Bound;
use std::sync::Arc;
use std::sync::atomic::Ordering;
// Third Party Imports
use serde::Serialize;
// Local Imports
use super::{Entry, OperationManager};
use crate::operations::Contract;
use crate::operations::caller::Source;
use crate::operations::host::{Catalog, CatalogPage};
use crate::{Error, Identifier, Result};
use harmos_domain::plugin::Grant;

/* === Constants === */
const MAX_ROWS: usize = 256;
const MAX_BYTES: usize = 4 * 1024 * 1024;
const CURSOR_BYTES: usize = MAX_BYTES;

/* === Macros === */

/* === Type Aliases === */
type ProviderEntry = (String, String, Arc<Entry>);

/* === Structs === */
#[derive(Serialize)]
struct Metadata<'a> {
    owner: &'a Identifier,
    generation: u64,
    contract: &'a Contract,
    grants: &'a [Grant],
    conditional_access: bool,
    grpc_available: bool,
}

struct Bounded {
    bytes: Vec<u8>,
    limit: usize,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */
impl Write for Bounded {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if bytes.len() > self.limit.saturating_sub(self.bytes.len()) {
            return Err(io::Error::other("discovery byte limit"));
        }
        self.bytes.extend_from_slice(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl OperationManager {
    pub(in crate::operations) fn discover(&self, source: &Source, request: &Catalog) -> Result<CatalogPage> {
        let origin = source.check()?;
        let (rows, bytes) = limits(request)?;
        let after: Option<(String, String)> = request
            .after
            .as_ref()
            .map(|cursor| {
                if cursor.len() > CURSOR_BYTES {
                    return Err(Error::InvalidInput { field: "discovery cursor", reason: "cursor is too large" });
                }
                let key: (String, String) = serde_json::from_str(cursor)
                    .map_err(|_| Error::InvalidInput { field: "discovery cursor", reason: "cursor must contain an operation and owner" })?;
                for value in [&key.0, &key.1] {
                    Identifier::new(value.as_str())
                        .map_err(|_| Error::InvalidInput { field: "discovery cursor", reason: "cursor contains an invalid identifier" })?;
                }
                Ok(key)
            })
            .transpose()?;

        let mut output = Bounded { bytes: vec![b'['], limit: bytes - 1 };
        let mut count = 0;
        let mut last = None;
        let mut next = None;
        let mut position = after;
        while let Some((id, owner, entry)) = self.next_discovery(position.as_ref())? {
            position = Some((id.clone(), owner.clone()));
            if !entry.active.load(Ordering::Acquire) || entry.available().is_err() || entry.authorize(&origin).is_err() {
                continue;
            }
            #[cfg(any(feature = "plugin", feature = "plugin-guest", feature = "plugin-sidecar"))]
            if entry.sdk.is_some() || entry.plugin.as_ref().is_some_and(|scope| !scope.is_ready() || scope.is_closed()) {
                continue;
            }
            let Some(contract) = &entry.contract else {
                continue;
            };
            let required = match self.required_external(&entry, &origin) {
                Ok(required) => required,
                Err(Error::OperationUnauthorized { .. } | Error::PermissionDenied) => continue,
                Err(error) => return Err(error),
            };
            // The Callback Can Race Reload/Shutdown; Never Report The Retired Provider.
            source.check()?;
            if !entry.active.load(Ordering::Acquire) || entry.available().is_err() {
                continue;
            }
            let state = self.registry.state.lock().expect("operation registry lock is healthy");
            if state.closing || !self.instances.is_accepting() {
                return Err(Error::Closing { domain: "operations" });
            }
            if !state.entries.get(&id).and_then(|owners| owners.get(&owner)).is_some_and(|registered| Arc::ptr_eq(registered, &entry)) {
                continue;
            }
            #[cfg(not(feature = "grpc"))]
            let grpc_available = false;
            #[cfg(feature = "grpc")]
            let grpc_available = entry.grpc.is_some()
                && contract.grpc().is_some_and(|binding| {
                    state.grpc.get(&binding.path()).is_some_and(|registered| registered.as_ptr() == Arc::as_ptr(&entry))
                });
            drop(state);
            let grants: &[Grant] = required.as_ref().map_or(&[], |selected| selected.grants.as_slice());
            #[cfg(any(feature = "plugin", feature = "plugin-guest", feature = "plugin-sidecar"))]
            let grants = origin
                .scope
                .as_ref()
                .and_then(|caller| caller.requirement(&entry.definition.id))
                .map_or(grants, |required| required.grants.as_slice());
            let selected_grants;
            let grants = if let Some(required) = &required {
                #[cfg(any(feature = "plugin", feature = "plugin-guest", feature = "plugin-sidecar"))]
                let plugin_required = origin.scope.as_ref().and_then(|caller| caller.requirement(&entry.definition.id));
                #[cfg(not(any(feature = "plugin", feature = "plugin-guest", feature = "plugin-sidecar")))]
                let plugin_required: Option<&harmos_domain::inventory::contracts::Required> = None;
                selected_grants = required
                    .grants
                    .iter()
                    .filter(|grant| plugin_required.is_none_or(|selected| selected.grants(grant)))
                    .cloned()
                    .collect::<Vec<_>>();
                selected_grants.as_slice()
            } else {
                grants
            };
            let metadata = Metadata {
                owner: &entry.owner,
                generation: entry.generation.value(),
                contract,
                grants,
                conditional_access: true,
                grpc_available,
            };
            let start = output.bytes.len();
            if count > 0 {
                output.bytes.push(b',');
            }
            if count == rows || serde_json::to_writer(&mut output, &metadata).is_err() {
                output.bytes.truncate(start);
                if count == 0 {
                    return Err(Error::ResourceExhausted { resource: "discovery schema bytes" });
                }
                next = last.take();
                break;
            }
            last = Some(serde_json::to_string(&(id, owner)).expect("identifier cursor serializes"));
            count += 1;
        }
        output.bytes.push(b']');
        Ok(CatalogPage { entries: output.bytes, next })
    }

    fn next_discovery(&self, after: Option<&(String, String)>) -> Result<Option<ProviderEntry>> {
        let state = self.registry.state.lock().expect("operation registry lock is healthy");
        if state.closing || !self.instances.is_accepting() {
            return Err(Error::Closing { domain: "operations" });
        }
        let start = after.map_or(Bound::Unbounded, |key| Bound::Included(key.0.as_str()));
        for (id, owners) in state.entries.range::<str, _>((start, Bound::Unbounded)) {
            let start = after.filter(|key| key.0 == *id).map_or(Bound::Unbounded, |key| Bound::Excluded(key.1.as_str()));
            if let Some((owner, entry)) = owners.range::<str, _>((start, Bound::Unbounded)).next() {
                return Ok(Some((id.clone(), owner.clone(), entry.clone())));
            }
        }
        Ok(None)
    }
}

/* === Public Functions === */
// The Row And Byte Bounds Of One Catalog Page, Shared By Every Kind.
pub(in crate::operations) fn limits(request: &Catalog) -> Result<(usize, usize)> {
    let rows = request.rows.unwrap_or(64);
    let bytes = request.bytes.unwrap_or(64 * 1024);
    if rows == 0 || rows > MAX_ROWS || !(2..=MAX_BYTES).contains(&bytes) {
        return Err(Error::InvalidInput { field: "catalog limits", reason: "rows must be 1..=256 and bytes must be 2..=4194304" });
    }
    Ok((rows, bytes))
}

/* === Private Functions === */

/* === Tests === */
