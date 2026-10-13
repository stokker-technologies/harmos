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
use std::sync::atomic::Ordering;
// Third Party Imports
use serde::Serialize;
// Local Imports
use super::OperationManager;
use crate::operations::Contract;
use crate::operations::caller::Source;
use crate::operations::host::{OperationsRequest, OperationsResponse};
use crate::{Error, Identifier, Result};
use harmos_contract::plugin::Grant;

/* === Constants === */
const MAX_ROWS: u32 = 256;
const MAX_BYTES: u32 = 4 * 1024 * 1024;
const CURSOR_BYTES: usize = MAX_BYTES as usize;

/* === Macros === */

/* === Type Aliases === */

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
    pub(in crate::operations) fn discover(
        &self,
        source: &Source,
        request: &OperationsRequest,
    ) -> Result<OperationsResponse> {
        let origin = source.check()?;
        let rows = request.rows.unwrap_or(64);
        let bytes = request.bytes.unwrap_or(64 * 1024);
        if rows == 0 || rows > MAX_ROWS || !(2..=MAX_BYTES).contains(&bytes) {
            return Err(Error::InvalidInput {
                field: "discovery limits",
                reason: "rows must be 1..=256 and bytes must be 2..=4194304",
            });
        }
        let after: Option<(String, String)> = request
            .after
            .as_ref()
            .map(|cursor| {
                if cursor.len() > CURSOR_BYTES {
                    return Err(Error::InvalidInput {
                        field: "discovery cursor",
                        reason: "cursor is too large",
                    });
                }
                let key: (String, String) =
                    serde_json::from_str(cursor).map_err(|_| Error::InvalidInput {
                        field: "discovery cursor",
                        reason: "cursor must contain an operation and owner",
                    })?;
                for value in [&key.0, &key.1] {
                    Identifier::new(value.as_str()).map_err(|_| Error::InvalidInput {
                        field: "discovery cursor",
                        reason: "cursor contains an invalid identifier",
                    })?;
                }
                Ok(key)
            })
            .transpose()?;

        let state = self
            .registry
            .state
            .lock()
            .expect("operation registry lock is healthy");
        if state.closing || !self.instances.is_accepting() {
            return Err(Error::Closing {
                domain: "operations",
            });
        }
        let mut output = Bounded {
            bytes: vec![b'['],
            limit: bytes as usize - 1,
        };
        let mut count = 0;
        let mut last = None;
        let mut next = None;
        'providers: for (id, owners) in &state.entries {
            for (owner, entry) in owners {
                if after
                    .as_ref()
                    .is_some_and(|key| (id, owner) <= (&key.0, &key.1))
                {
                    continue;
                }
                if !entry.active.load(Ordering::Acquire)
                    || entry.available().is_err()
                    || entry.authorize(&origin).is_err()
                {
                    continue;
                }
                #[cfg(any(
                    feature = "plugin",
                    feature = "plugin-guest",
                    feature = "plugin-sidecar"
                ))]
                if entry.sdk.is_some()
                    || entry
                        .plugin
                        .as_ref()
                        .is_some_and(|scope| !scope.is_ready() || scope.is_closed())
                {
                    continue;
                }
                let Some(contract) = &entry.contract else {
                    continue;
                };
                let grants: &[Grant] = &[];
                #[cfg(any(
                    feature = "plugin",
                    feature = "plugin-guest",
                    feature = "plugin-sidecar"
                ))]
                let grants = origin
                    .scope
                    .as_ref()
                    .and_then(|caller| caller.requirement(&entry.definition.id))
                    .map_or(grants, |required| required.grants.as_slice());
                #[cfg(not(feature = "grpc"))]
                let grpc_available = false;
                #[cfg(feature = "grpc")]
                let grpc_available = entry.grpc.is_some()
                    && contract.grpc().is_some_and(|binding| {
                        state.grpc.get(&binding.path()).is_some_and(|registered| {
                            registered.as_ptr() == std::sync::Arc::as_ptr(entry)
                        })
                    });
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
                        return Err(Error::ResourceExhausted {
                            resource: "discovery schema bytes",
                        });
                    }
                    next = last.take();
                    break 'providers;
                }
                last = Some(
                    serde_json::to_string(&(id, owner)).expect("identifier cursor serializes"),
                );
                count += 1;
            }
        }
        output.bytes.push(b']');
        Ok(OperationsResponse {
            catalog: output.bytes,
            next,
        })
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
