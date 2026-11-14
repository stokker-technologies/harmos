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
use std::io::Write;

// Third Party Imports
use harmos_operations::execution::operations::ServingError;
use harmos_protocol::client::status::{DiagnosticsGetRequest, DiagnosticsGetResponse};
use sha2::{Digest, Sha256};

// Local Imports
use super::{Status, failure};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

pub struct Entry {
    pub path: String,
    pub text: String,
}
pub(super) struct Archive {
    pub name: String,
    bytes: std::sync::Arc<Vec<u8>>,
    digest: [u8; 32],
    clock: crate::clock::Clock,
    timeout: std::time::Duration,
    maximum: std::num::NonZeroUsize,
}
struct Bounded {
    bytes: Vec<u8>,
    maximum: usize,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl harmos_storage::ContentSource for Archive {
    fn resolve<'a>(&'a self, reference: &'a str, _: Option<&'a str>) -> harmos_storage::StorageFuture<'a, harmos_storage::ContentLease> {
        Box::pin(async move {
            if reference != self.name {
                return Err(harmos_storage::Error::InvalidInput {
                    field: "Status.Diagnostics.Get",
                    reason: "archive is unknown or expired",
                });
            }
            let expires_at = self.clock.read().date.checked_add(self.timeout).ok_or(harmos_storage::Error::InvalidInput {
                field: "Status.Diagnostics.Get",
                reason: "read_timeout exceeds clock capacity",
            })?;
            Ok(harmos_storage::ContentLease {
                length: self.bytes.len() as u64,
                sha256: self.digest,
                expires_at,
                max_bytes: self.maximum,
                token: self.name.as_bytes().to_vec(),
            })
        })
    }
    fn read<'a>(
        &'a self, lease: &'a harmos_storage::ContentLease, offset: u64, maximum: std::num::NonZeroUsize,
    ) -> harmos_storage::StorageFuture<'a, Vec<u8>> {
        Box::pin(async move {
            if lease.token != self.name.as_bytes() || lease.length != self.bytes.len() as u64 || lease.sha256 != self.digest {
                return Err(harmos_storage::Error::InvalidInput {
                    field: "Status.Diagnostics.Get",
                    reason: "archive lease does not match content",
                });
            }
            let offset =
                usize::try_from(offset).ok().filter(|offset| *offset <= self.bytes.len()).ok_or(harmos_storage::Error::InvalidInput {
                    field: "Status.Diagnostics.Get",
                    reason: "offset exceeds content length",
                })?;
            let end = offset.saturating_add(maximum.min(self.maximum).get()).min(self.bytes.len());
            Ok(self.bytes[offset..end].to_vec())
        })
    }
}

impl Write for Bounded {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        if self.bytes.len().saturating_add(bytes.len()) > self.maximum {
            return Err(std::io::Error::other("archive byte budget exceeded"));
        }
        self.bytes.extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}
impl Status {
    pub(super) async fn diagnostics(&self, request: DiagnosticsGetRequest) -> Result<DiagnosticsGetResponse, ServingError> {
        let action = "Status.Diagnostics.Get";
        if request.max_bytes == 0 || request.max_bytes > self.settings.range_bytes.get() as u64 {
            return Err(failure(action, "range exceeds configured byte bound"));
        }
        let name = if request.name.is_empty() {
            if request.offset != 0 {
                return Err(failure(action, "first archive range must start at zero"));
            }
            self.archive()?
        } else {
            request.name
        };
        let archive = {
            let state = self.state.lock().expect("status lock is healthy");
            state
                .archive
                .as_ref()
                .filter(|archive| archive.name == name)
                .cloned()
                .ok_or_else(|| failure(action, "archive is unknown or expired"))?
        };
        let read = harmos_storage::ContentRead::open(archive, &name, None, self.clock.clone(), self.settings.archive_bytes)
            .await
            .map_err(|error| failure(action, &error.to_string()))?;
        let range = read
            .get_bytes(request.offset, std::num::NonZeroUsize::new(request.max_bytes as usize).expect("validated positive range"))
            .await
            .map_err(|error| failure(action, &error.to_string()))?;
        Ok(DiagnosticsGetResponse {
            name,
            data: range.data,
            total_length: range.total_length,
            sha256: range.sha256.to_vec(),
        })
    }
    fn archive(&self) -> Result<String, ServingError> {
        let action = "Status.Diagnostics.Get";
        let (status, providers, logs, traces) = {
            let mut state = self.state.lock().expect("status lock is healthy");
            let page = harmos_protocol::v1::PageRequest {
                page_size: u32::MAX,
                max_bytes: self
                    .settings
                    .history_bytes
                    .get()
                    .saturating_add(self.settings.history_items.get().saturating_mul(16))
                    .saturating_add(128) as u64,
                page_token: String::new(),
            };
            let logs = state.logs.page(None, &page, action)?.0;
            let traces = state.traces.page(None, &page, action)?.0;
            (self.snapshot(&state), state.providers.values().cloned().collect::<Vec<_>>(), logs, traces)
        };
        let mut entries = vec![
            Entry {
                path: "host.txt".into(),
                text: format!("{status:#?}"),
            },
            Entry {
                path: "logs.txt".into(),
                text: format!("{logs:#?}"),
            },
            Entry {
                path: "trace-events.txt".into(),
                text: format!("{traces:#?}"),
            },
        ];
        for provider in providers {
            entries.extend(provider.diagnostics().map_err(|_| failure(action, "contribution unavailable"))?);
        }
        let mut paths = std::collections::BTreeSet::new();
        let mut builder = tar::Builder::new(Bounded {
            bytes: Vec::new(),
            maximum: self.settings.archive_bytes.get(),
        });
        for mut entry in entries {
            entry.path = self.redaction.text(&entry.path);
            if entry.path.is_empty()
                || entry.path.starts_with('/')
                || entry.path.contains('\\')
                || entry.path.split('/').any(|part| part.is_empty() || part == ".." || part == ".")
                || entry.path.contains(':')
                || !paths.insert(entry.path.clone())
            {
                return Err(failure(action, "archive entry path is unsafe or duplicated"));
            }
            let text = self.redaction.text(&entry.text);
            let mut header = tar::Header::new_gnu();
            header.set_size(text.len() as u64);
            header.set_mode(0o600);
            header.set_mtime(0);
            header.set_cksum();
            builder
                .append_data(&mut header, &entry.path, text.as_bytes())
                .map_err(|_| failure(action, "archive entry exceeds configured budget or has an invalid path"))?;
        }
        let bytes = builder.into_inner().map_err(|_| failure(action, "archive exceeds configured byte budget"))?.bytes;
        let digest: [u8; 32] = Sha256::digest(&bytes).into();
        let name = digest.iter().map(|byte| format!("{byte:02x}")).collect::<String>();
        self.state.lock().expect("status lock is healthy").archive = Some(std::sync::Arc::new(Archive {
            name: name.clone(),
            bytes: std::sync::Arc::new(bytes),
            digest,
            clock: self.clock.clone(),
            timeout: self.settings.read_timeout,
            maximum: self.settings.range_bytes,
        }));
        Ok(name)
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
