/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */
mod draft;
mod fact;
mod vault;

/* === Re-Exports === */
pub use draft::Draft;
pub(crate) use fact::SOURCE;
pub use fact::{EvidenceKind, EvidenceRecord};
pub use vault::{Excerpt, Vault};

/* === Imports === */
// Standard Library Imports
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex, Weak};
// Third Party Imports
use tokio::sync::watch;
// Local Imports
use crate::supervision::executor;
use crate::{
    Application, DefinitionId, state::At, state::Entry, state::Recorded, state::StateManager,
};
use draft::{Open, Staged};
use vault::Keeping;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */
type Held<T> = BTreeMap<String, BTreeMap<(DefinitionId, String), T>>;

/* === Structs === */
pub(crate) struct Declaration {
    vault: Arc<dyn Keeping>,
    kinds: Vec<(DefinitionId, u32)>,
}

pub struct Evidence<A: Application> {
    inner: Option<Arc<Inner<A>>>,
}

struct Inner<A: Application> {
    state: StateManager<A>,
    vault: Arc<dyn Keeping>,
    kinds: BTreeMap<DefinitionId, u32>,
    drafts: Mutex<Held<Arc<Open>>>,
    kept: Mutex<Held<EvidenceRecord>>,
}

/* === Enums === */
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum EvidenceError {
    #[error("this runtime was assembled without an evidence vault")]
    Unserved,
    #[error("`{kind}` is not a declared evidence kind")]
    Undeclared { kind: DefinitionId },
    #[error("this evidence vault hands out no external draft paths")]
    Unsupported,
    #[error("this draft is written by a process outside the runtime")]
    Foreign,
    #[error("`{scope}`/`{key}` already names evidence of kind `{kind}`")]
    Conflict {
        kind: DefinitionId,
        scope: String,
        key: String,
    },
    #[error("no evidence is open or finalized under this key")]
    Unknown,
    #[error("`{named}` does not declare the details of `{kind}` evidence")]
    Mismatched {
        kind: DefinitionId,
        named: DefinitionId,
    },
    #[error("the details of `{kind}` do not match its declaration: {cause}")]
    Malformed { kind: DefinitionId, cause: String },
    #[error("this evidence is already being finalized")]
    Finalizing,
    #[error("a read of {requested} bytes is past this vault's {ceiling}-byte bound")]
    Bound { requested: usize, ceiling: usize },
    #[error("the stored evidence is {found} bytes where {written} were written")]
    Corrupt { written: u64, found: u64 },
    #[error("the evidence vault could not be read: {0}")]
    Unreadable(String),
    #[error("the evidence vault could not be written: {0}")]
    Unwritable(String),
    #[error("the state is unavailable, so nothing was finalized")]
    Unavailable,
}

/* === Traits === */

/* === Implementations === */
impl EvidenceError {
    pub fn unreadable(cause: impl std::fmt::Display) -> Self {
        Self::Unreadable(cause.to_string())
    }

    pub fn unwritable(cause: impl std::fmt::Display) -> Self {
        Self::Unwritable(cause.to_string())
    }
}

impl Declaration {
    pub(crate) fn new(
        vault: impl Vault,
        kinds: impl IntoIterator<Item = (DefinitionId, u32)>,
    ) -> Self {
        Self {
            vault: Arc::new(vault),
            kinds: kinds.into_iter().collect(),
        }
    }
}

impl<A: Application> Evidence<A> {
    pub(crate) fn wired(
        state: StateManager<A>,
        declaration: Option<Declaration>,
        recovered: &[Arc<Entry<Recorded<A>>>],
    ) -> Result<Self, String> {
        let Some(Declaration { vault, kinds }) = declaration else {
            return Ok(Self { inner: None });
        };
        let mut frozen = BTreeMap::new();
        for (id, version) in kinds {
            if frozen.insert(id.clone(), version).is_some() {
                return Err(format!("evidence kind `{id}` is declared more than once"));
            }
        }
        let mut kept: Held<EvidenceRecord> = BTreeMap::new();
        for entry in recovered {
            let Recorded::System(fact) = &entry.value else {
                continue;
            };
            let Some(record) = EvidenceRecord::folded(fact, entry.position) else {
                continue;
            };
            kept.entry(record.scope.clone())
                .or_default()
                .insert((record.kind.clone(), record.key.clone()), record);
        }

        Ok(Self {
            inner: Some(Arc::new(Inner {
                state,
                vault,
                kinds: frozen,
                drafts: Mutex::new(BTreeMap::new()),
                kept: Mutex::new(kept),
            })),
        })
    }

    pub async fn open<K: EvidenceKind>(
        &self,
        scope: impl Into<String>,
        key: impl Into<String>,
        details: K::Details,
    ) -> Result<Draft, EvidenceError> {
        self.opening::<K>(scope.into(), key.into(), details, false)
            .await
    }

    pub async fn external<K: EvidenceKind>(
        &self,
        scope: impl Into<String>,
        key: impl Into<String>,
        details: K::Details,
    ) -> Result<Draft, EvidenceError> {
        self.opening::<K>(scope.into(), key.into(), details, true)
            .await
    }

    pub async fn read<K: EvidenceKind>(
        &self,
        scope: &str,
        key: &str,
        offset: u64,
        limit: usize,
    ) -> Result<Excerpt, EvidenceError> {
        let inner = self.inner()?;
        let kind = inner.declared::<K>()?;
        let ceiling = inner.vault.ceiling();
        if limit == 0 || limit > ceiling {
            return Err(EvidenceError::Bound {
                requested: limit,
                ceiling,
            });
        }

        let staged = inner.draft(&kind, scope, key);
        let (bytes, length) = match staged {
            Some(open) => {
                let read = inner.vault.read_draft(&open.name, offset, limit).await?;
                open.observed(read.1);
                read
            }
            None => {
                let name = encode(&kind, scope, key);
                if !inner.is_kept(&kind, scope, key) {
                    return Err(EvidenceError::Unknown);
                }
                inner.vault.read(&name, offset, limit).await?
            }
        };

        Ok(Excerpt {
            requested: offset,
            offset: offset.min(length),
            bytes,
            length,
        })
    }

    pub fn list(&self, scope: &str) -> Vec<EvidenceRecord> {
        let Ok(inner) = self.inner() else {
            return Vec::new();
        };
        let mut listing: BTreeMap<(DefinitionId, String), EvidenceRecord> = BTreeMap::new();

        for (at, open) in inner.scope(&inner.drafts, scope) {
            listing.insert(
                at,
                EvidenceRecord::new(
                    open.kind.clone(),
                    open.version,
                    open.scope.clone(),
                    open.key.clone(),
                    open.length(),
                    open.described.clone(),
                ),
            );
        }
        for (at, record) in inner.scope(&inner.kept, scope) {
            listing.insert(at, record);
        }

        listing.into_values().collect()
    }

    pub async fn finalize(&self, draft: &Draft) -> Result<EvidenceRecord, EvidenceError> {
        let inner = self.inner()?;
        let open = &draft.open;
        inner.begin(open)?;

        let finalized = inner.finalizing(open).await;
        inner.end(open, finalized.is_ok());

        finalized
    }

    pub async fn abandon(&self, draft: &Draft) -> Result<(), EvidenceError> {
        let inner = self.inner()?;
        let open = &draft.open;
        inner.begin(open)?;

        let quarantined = inner.vault.quarantine(&open.name).await;
        inner.forget(open);
        open.release(open.length());

        quarantined
    }

    pub async fn finalize_scope(&self, scope: &str) -> Result<Vec<EvidenceRecord>, EvidenceError> {
        let inner = self.inner()?;
        let mut finalized = Vec::new();

        for (_, open) in inner.scope(&inner.drafts, scope) {
            finalized.push(self.finalize(&Draft { open }).await?);
        }

        Ok(finalized)
    }

    pub async fn abandon_scope(&self, scope: &str) -> Result<(), EvidenceError> {
        let inner = self.inner()?;

        for (_, open) in inner.scope(&inner.drafts, scope) {
            self.abandon(&Draft { open }).await?;
        }

        Ok(())
    }

    pub fn watch(&self, draft: &Draft) -> Result<watch::Receiver<u64>, EvidenceError> {
        let inner = self.inner()?;
        let open = &draft.open;
        if open.released() {
            return Err(EvidenceError::Unknown);
        }
        let (receiver, first) = open.watch();
        if first && open.external.is_some() {
            let cadence = inner.vault.cadence();
            executor::task(observing(Arc::downgrade(open), cadence));
        }

        Ok(receiver)
    }

    async fn opening<K: EvidenceKind>(
        &self,
        scope: String,
        key: String,
        details: K::Details,
        foreign: bool,
    ) -> Result<Draft, EvidenceError> {
        let inner = self.inner()?;
        let kind = inner.declared::<K>()?;
        let version = inner.kinds[&kind];
        let described = serde_json::to_vec(&details).map_err(|error| EvidenceError::Malformed {
            kind: kind.clone(),
            cause: error.to_string(),
        })?;
        if inner.draft(&kind, &scope, &key).is_some() || inner.is_kept(&kind, &scope, &key) {
            return Err(EvidenceError::Conflict { kind, scope, key });
        }
        let name = encode(&kind, &scope, &key);
        let length = inner.vault.open(&name).await?;
        let external = match foreign {
            true => Some(
                inner
                    .vault
                    .external(&name)
                    .await?
                    .ok_or(EvidenceError::Unsupported)?,
            ),
            false => None,
        };
        let open = Arc::new(Open::new(
            Staged {
                kind: kind.clone(),
                version,
                scope: scope.clone(),
                key: key.clone(),
                name,
                described,
                external,
                length,
            },
            Arc::clone(&inner.vault),
        ));
        inner
            .drafts
            .lock()
            .expect("the evidence index is healthy")
            .entry(scope)
            .or_default()
            .insert((kind, key), Arc::clone(&open));

        Ok(Draft { open })
    }

    fn inner(&self) -> Result<&Arc<Inner<A>>, EvidenceError> {
        self.inner.as_ref().ok_or(EvidenceError::Unserved)
    }
}

impl<A: Application> Clone for Evidence<A> {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
        }
    }
}

impl<A: Application> Inner<A> {
    fn declared<K: EvidenceKind>(&self) -> Result<DefinitionId, EvidenceError> {
        let (kind, _) = K::definition();
        match self.kinds.contains_key(&kind) {
            true => Ok(kind),
            false => Err(EvidenceError::Undeclared { kind }),
        }
    }

    fn draft(&self, kind: &DefinitionId, scope: &str, key: &str) -> Option<Arc<Open>> {
        self.drafts
            .lock()
            .expect("the evidence index is healthy")
            .get(scope)?
            .get(&(kind.clone(), key.to_owned()))
            .map(Arc::clone)
    }

    fn is_kept(&self, kind: &DefinitionId, scope: &str, key: &str) -> bool {
        self.kept
            .lock()
            .expect("the evidence index is healthy")
            .get(scope)
            .is_some_and(|held| held.contains_key(&(kind.clone(), key.to_owned())))
    }

    fn scope<T: Clone>(
        &self,
        index: &Mutex<Held<T>>,
        scope: &str,
    ) -> Vec<((DefinitionId, String), T)> {
        index
            .lock()
            .expect("the evidence index is healthy")
            .get(scope)
            .map(|held| {
                held.iter()
                    .map(|(at, value)| (at.clone(), value.clone()))
                    .collect()
            })
            .unwrap_or_default()
    }

    fn begin(&self, open: &Arc<Open>) -> Result<(), EvidenceError> {
        match self.draft(&open.kind, &open.scope, &open.key) {
            Some(held) if Arc::ptr_eq(&held, open) => match open.begin_finalize() {
                true => Ok(()),
                false => Err(EvidenceError::Finalizing),
            },
            Some(_) | None => Err(EvidenceError::Unknown),
        }
    }

    fn end(&self, open: &Arc<Open>, ended: bool) {
        if !ended {
            open.end_finalize();
        }
    }

    async fn finalizing(&self, open: &Arc<Open>) -> Result<EvidenceRecord, EvidenceError> {
        // Durable first, then read back: a digest folded from what a producer
        // said it wrote proves nothing about what survived.
        let (digest, length) = self.vault.verify(&open.name).await?;
        let written = open.length();
        if open.external.is_none() && written != length {
            return Err(EvidenceError::Corrupt {
                written,
                found: length,
            });
        }

        let mut record = EvidenceRecord::new(
            open.kind.clone(),
            open.version,
            open.scope.clone(),
            open.key.clone(),
            length,
            open.described.clone(),
        );
        let receipt = crate::records::RecordManager::new(self.state.clone())
            .publish_system(record.recorded(digest))
            .await
            .map_err(|_| EvidenceError::Unavailable)?;
        self.state.wait_until_stored(receipt.position).await;
        record.finalized = Some(At {
            value: digest,
            position: receipt.position,
        });

        // Visible before removed: the fact is already history, so the
        // evidence enters the finalized index before its draft leaves the
        // active one and a concurrent listing never falls through the gap
        // between them.
        self.kept
            .lock()
            .expect("the evidence index is healthy")
            .entry(record.scope.clone())
            .or_default()
            .insert((record.kind.clone(), record.key.clone()), record.clone());
        self.vault.claim(&open.name).await?;
        self.forget(open);
        open.release(length);

        Ok(record)
    }

    fn forget(&self, open: &Arc<Open>) {
        let mut drafts = self.drafts.lock().expect("the evidence index is healthy");
        let Some(held) = drafts.get_mut(&open.scope) else {
            return;
        };
        held.remove(&(open.kind.clone(), open.key.clone()));
        if held.is_empty() {
            drafts.remove(&open.scope);
        }
    }
}

/* === Public Functions === */

/* === Private Functions === */
async fn observing(draft: Weak<Open>, cadence: std::time::Duration) {
    loop {
        executor::sleep(cadence).await;
        let Some(open) = draft.upgrade() else {
            return;
        };
        if open.released() || open.idle() {
            return;
        }
        if open.observe().await.is_err() {
            return;
        }
    }
}

fn encode(kind: &DefinitionId, scope: &str, key: &str) -> String {
    let mut name = String::new();
    for (index, part) in [kind.as_str(), scope, key].into_iter().enumerate() {
        if index > 0 {
            name.push('~');
        }
        for byte in part.bytes() {
            match byte {
                b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' => name.push(byte as char),
                _ => name.push_str(&format!("%{byte:02x}")),
            }
        }
    }

    name
}

/* === Tests === */
