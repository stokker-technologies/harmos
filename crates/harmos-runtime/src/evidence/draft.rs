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
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
// Third Party Imports
use tokio::sync::watch;
// Local Imports
use super::EvidenceError;
use super::vault::Keeping;
use crate::DefinitionId;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

#[derive(Clone)]
pub struct Draft {
    pub(crate) open: Arc<Open>,
}

pub(crate) struct Open {
    pub(crate) kind: DefinitionId,
    pub(crate) version: u32,
    pub(crate) scope: String,
    pub(crate) key: String,
    pub(crate) name: String,
    pub(crate) described: Vec<u8>,
    pub(crate) external: Option<PathBuf>,
    vault: Arc<dyn Keeping>,
    writing: tokio::sync::Mutex<()>,
    growth: Mutex<Growth>,
}

struct Growth {
    length: u64,
    watchers: Option<watch::Sender<u64>>,
    finalizing: bool,
    released: bool,
}

pub(crate) struct Staged {
    pub(crate) kind: DefinitionId,
    pub(crate) version: u32,
    pub(crate) scope: String,
    pub(crate) key: String,
    pub(crate) name: String,
    pub(crate) described: Vec<u8>,
    pub(crate) external: Option<PathBuf>,
    pub(crate) length: u64,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl Draft {
    pub fn kind(&self) -> &DefinitionId {
        &self.open.kind
    }

    pub fn scope(&self) -> &str {
        &self.open.scope
    }

    pub fn key(&self) -> &str {
        &self.open.key
    }

    pub fn length(&self) -> u64 {
        self.open.length()
    }

    pub fn path(&self) -> Option<&Path> {
        self.open.external.as_deref()
    }

    pub async fn write(&self, bytes: &[u8]) -> Result<u64, EvidenceError> {
        self.open.append(bytes).await
    }
}

impl std::fmt::Debug for Draft {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Draft")
            .field("kind", &self.open.kind)
            .field("scope", &self.open.scope)
            .field("key", &self.open.key)
            .field("length", &self.open.length())
            .finish()
    }
}

impl Open {
    pub(crate) fn new(staged: Staged, vault: Arc<dyn Keeping>) -> Self {
        let Staged {
            kind,
            version,
            scope,
            key,
            name,
            described,
            external,
            length,
        } = staged;

        Self {
            kind,
            version,
            scope,
            key,
            name,
            described,
            external,
            vault,
            writing: tokio::sync::Mutex::new(()),
            growth: Mutex::new(Growth {
                length,
                watchers: None,
                finalizing: false,
                released: false,
            }),
        }
    }

    pub(crate) fn length(&self) -> u64 {
        self.held().length
    }

    pub(crate) fn released(&self) -> bool {
        self.held().released
    }

    async fn append(&self, bytes: &[u8]) -> Result<u64, EvidenceError> {
        if self.external.is_some() {
            return Err(EvidenceError::Foreign);
        }
        let _writing = self.writing.lock().await;
        if self.released() {
            return Err(EvidenceError::Unknown);
        }
        let length = self.vault.append(&self.name, bytes).await?;
        self.observed(length);

        Ok(length)
    }

    pub(crate) fn observed(&self, length: u64) {
        let mut held = self.held();
        if held.length == length {
            return;
        }
        held.length = length;
        if let Some(watchers) = &held.watchers {
            let _ = watchers.send(length);
        }
    }

    pub(crate) fn watch(&self) -> (watch::Receiver<u64>, bool) {
        let mut held = self.held();
        match &held.watchers {
            Some(watchers) => (watchers.subscribe(), false),
            None => {
                let (sender, receiver) = watch::channel(held.length);
                held.watchers = Some(sender);

                (receiver, true)
            }
        }
    }

    pub(crate) fn idle(&self) -> bool {
        let mut held = self.held();
        let gone = held.watchers.as_ref().is_some_and(|watchers| watchers.receiver_count() == 0);
        if gone {
            held.watchers = None;
        }

        gone
    }

    pub(crate) fn begin_finalize(&self) -> bool {
        let mut held = self.held();
        if held.finalizing || held.released {
            return false;
        }
        held.finalizing = true;

        true
    }

    pub(crate) fn end_finalize(&self) {
        self.held().finalizing = false;
    }

    pub(crate) async fn observe(&self) -> Result<(), EvidenceError> {
        let length = self.vault.length(&self.name).await?;
        self.observed(length);

        Ok(())
    }

    pub(crate) fn release(&self, length: u64) {
        let mut held = self.held();
        held.released = true;
        held.length = length;
        if let Some(watchers) = held.watchers.take() {
            let _ = watchers.send(length);
        }
    }

    fn held(&self) -> std::sync::MutexGuard<'_, Growth> {
        self.growth.lock().expect("the draft lock is healthy")
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
