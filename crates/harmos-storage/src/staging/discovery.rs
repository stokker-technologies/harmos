/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Discover Unfinished File Productions

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::mem;

// Third Party Imports

// Local Imports
use super::StorageManager;
use crate::{Error, FileSession, Identifier, MemorySize, ReadLimits, Result, StorageCursor};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/// An Unfinished Production Owned by the Existing Staging and Recovery State.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileProduction {
    pub session: FileSession,
    pub accepted_length: u64,
    pub durable_length: u64,
}

#[derive(Clone, Debug)]
pub struct FileProductions {
    pub productions: Vec<FileProduction>,
    pub next: Option<Identifier>,
    pub at: StorageCursor,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl MemorySize for FileProduction {
    fn memory_size(&self) -> usize {
        mem::size_of::<Self>() + self.session.memory_size() - mem::size_of::<FileSession>()
    }
}

impl StorageManager {
    /// Discover Live Productions in Exact Scope and Identity Order.
    /// Admission Must Be Fenced by the Consumer for a Complete Traversal.
    pub fn list_file_productions(
        &self, scope: &Identifier, after: Option<&Identifier>, at: Option<&StorageCursor>, limits: ReadLimits,
    ) -> Result<FileProductions> {
        let state = self.inner.state.lock().unwrap();
        if state.status.closing || !state.ready {
            return Err(Error::NotReady {
                domain: "file discovery; start storage before listing productions",
            });
        }
        let at = at.unwrap_or(&state.status.saved);
        if !state.status.saved.covers(at)? {
            return Err(Error::InvalidInput {
                field: "file discovery cut",
                reason: "wait until storage has saved the requested position",
            });
        }
        // Recovery Restores This Same Set Before Storage Becomes Ready.
        let mut files: Vec<_> = state
            .files
            .values()
            .filter(|file| {
                file.session.is_some()
                    && file.belongs_to_scope(scope)
                    && after.is_none_or(|after| file.staging.id > *after)
                    && file.progress.lock().unwrap().ready
            })
            .collect();
        files.sort_unstable_by(|left, right| left.staging.id.cmp(&right.staging.id));
        let rows = limits.rows.min(self.inner.settings.response.batch_rows).get();
        let maximum = limits.bytes.min(self.inner.settings.response.batch_bytes).get();
        let mut productions: Vec<FileProduction> = Vec::new();
        let mut bytes = 0usize;
        let mut next = None;
        for file in files {
            let status = file.production_status();
            let production = FileProduction {
                session: FileSession {
                    history: file.staging.history.clone(),
                    id: file.staging.id.clone(),
                    generation: status.writer_generation.expect("selected resumable production"),
                },
                accepted_length: status.accepted_length,
                durable_length: status.durable_length,
            };
            let size = production.memory_size();
            if productions.len() == rows || bytes.saturating_add(size) > maximum {
                let Some(last) = productions.last() else {
                    return Err(Error::ResourceExhausted {
                        resource: "file discovery page; increase the response byte budget",
                    });
                };
                next = Some(last.session.id.clone());
                break;
            }
            bytes += size;
            productions.push(production);
        }
        Ok(FileProductions { productions, next, at: at.clone() })
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
