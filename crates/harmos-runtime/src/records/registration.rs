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
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};
// Third Party Imports

// Local Imports
use crate::schema::{Validator, graph::Schema};
use crate::storage::{ErasedCatalog, SchemaKey, StorageManager};
use crate::{Error, MemorySize, Result};

/* === Constants === */

const DEFINITIONS: usize = 64;
const REGISTRY_BYTES: usize = 8 * 1024 * 1024;
const NAME_BYTES: usize = 1024;

/* === Macros === */

/* === Type Aliases === */

type Key = (String, u32);

/* === Structs === */

#[derive(Debug)]
pub struct Definition {
    owner: String,
    schema: SchemaKey,
    graph: Arc<Schema>,
    validation: Validator,
}

pub(crate) struct Registry {
    pub(super) storage: Option<StorageManager<ErasedCatalog>>,
    state: Mutex<State>,

    #[cfg(any(feature = "plugin", feature = "plugin-guest", feature = "plugin-sidecar"))]
    pub(super) listeners: Mutex<BTreeMap<String, Arc<super::listeners::Listening>>>,
}

#[derive(Default)]
struct State {
    definitions: BTreeMap<Key, Entry>,
    bytes: usize,
    constructing_bytes: usize,
    constructing: usize,
}

struct Entry {
    definition: Arc<Definition>,
    bytes: usize,
    ready: bool,
}

struct Construction {
    registry: Arc<Registry>,
    bytes: usize,
}

pub(super) struct Pending {
    registry: Arc<Registry>,
    key: Key,
    pub(super) definition: Arc<Definition>,
    committed: bool,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl Definition {
    pub fn owner(&self) -> &str {
        &self.owner
    }

    pub fn schema(&self) -> &SchemaKey {
        &self.schema
    }

    pub fn graph(&self) -> &Schema {
        &self.graph
    }

    pub(crate) fn graph_owner(&self) -> Arc<Schema> {
        Arc::clone(&self.graph)
    }

    pub fn validate(&self, payload: &[u8]) -> Result<()> {
        self.validation.validate(payload).map_err(|source| Error::RecordPayload { source })
    }
}

impl Registry {
    pub(crate) fn new(storage: Option<StorageManager<ErasedCatalog>>) -> Self {
        Self {
            storage,
            state: Mutex::new(State::default()),
            #[cfg(any(feature = "plugin", feature = "plugin-guest", feature = "plugin-sidecar"))]
            listeners: Mutex::new(BTreeMap::new()),
        }
    }

    pub(super) fn reserve(self: &Arc<Self>, owner: &str, schema: SchemaKey, graph: Arc<Schema>) -> Result<Pending> {
        if !harmos_domain::artifact_subset(owner) || owner.len() > NAME_BYTES || schema.id.as_str().len() > NAME_BYTES {
            return Err(Error::InvalidInput {
                field: "record schema",
                reason: "invalid or oversized identity",
            });
        }
        let settings = harmos_domain::config::SchemaSettings::default();
        graph.preparation(&settings).map_err(|_| limit())?;
        let fields_bytes = graph.memory_size().checked_add(2 * std::mem::size_of::<usize>()).ok_or_else(limit)?;
        {
            let state = self.state.lock().expect("record schema registry poisoned");
            if let Some((key, entry)) =
                state.definitions.iter().find(|((id, version), _)| id == schema.id.as_str() && *version == schema.version.get())
            {
                if entry.ready && entry.definition.owner == owner && entry.definition.graph == graph {
                    return Ok(Pending {
                        registry: Arc::clone(self),
                        key: key.clone(),
                        definition: Arc::clone(&entry.definition),
                        committed: true,
                    });
                }
                return Err(Error::AlreadyRegistered {
                    domain: "record schema",
                    name: key.0.clone(),
                });
            }
            if state.definitions.values().any(|entry| entry.definition.schema.id == schema.id && entry.definition.owner != owner) {
                return Err(Error::AlreadyRegistered {
                    domain: "record schema owner",
                    name: schema.id.as_str().to_owned(),
                });
            }
        }
        let destination = [
            std::mem::size_of::<Definition>(),
            std::mem::size_of::<(Key, Entry)>(),
            1024,
            owner.len(),
            schema.id.as_str().len(),
            schema.id.as_str().len(),
            fields_bytes,
        ]
        .into_iter()
        .try_fold(0usize, usize::checked_add)
        .ok_or_else(limit)?;
        let compilation = Validator::construction_memory_bytes(&graph, &settings).map_err(|source| Error::RecordSchema {
            source: crate::schema::Error::Graph(source),
        })?;
        let construction_bytes = destination.checked_add(compilation).ok_or_else(limit)?;
        let mut construction = Construction::new(self, construction_bytes)?;
        // Compile Outside The Registry Lock Under The Original Shared Budget
        let validation = Validator::from_schema(&graph, &settings).map_err(|source| Error::RecordSchema {
            source: crate::schema::Error::Graph(source),
        })?;
        let bytes = destination.checked_add(validation.memory_size()).ok_or_else(limit)?;
        if bytes > construction_bytes {
            return Err(limit());
        }
        let key = (schema.id.as_str().to_owned(), schema.version.get());
        let definition = Arc::new(Definition {
            owner: owner.to_owned(),
            schema: schema.clone(),
            graph,
            validation,
        });
        let mut state = self.state.lock().expect("record schema registry poisoned");
        if state.definitions.contains_key(&key) {
            return Err(Error::AlreadyRegistered { domain: "record schema", name: key.0 });
        }
        if state.definitions.values().any(|entry| entry.definition.schema.id == schema.id && entry.definition.owner != owner) {
            return Err(Error::AlreadyRegistered {
                domain: "record schema owner",
                name: key.0,
            });
        }
        construction.transfer(&mut state, bytes);
        state.definitions.insert(
            key.clone(),
            Entry {
                definition: Arc::clone(&definition),
                bytes,
                ready: false,
            },
        );
        Ok(Pending {
            registry: Arc::clone(self),
            key,
            definition,
            committed: false,
        })
    }

    pub(super) fn definition(&self, schema: &SchemaKey) -> Option<Arc<Definition>> {
        if schema.id.as_str().len() > NAME_BYTES {
            return None;
        }
        let state = self.state.lock().expect("record schema registry poisoned");
        state
            .definitions
            .get(&(schema.id.as_str().to_owned(), schema.version.get()))
            .filter(|entry| entry.ready)
            .map(|entry| Arc::clone(&entry.definition))
    }

    pub(super) fn definitions(&self) -> Vec<Arc<Definition>> {
        self.state
            .lock()
            .expect("record schema registry poisoned")
            .definitions
            .values()
            .filter(|entry| entry.ready)
            .map(|entry| Arc::clone(&entry.definition))
            .collect()
    }
}

impl Construction {
    fn new(registry: &Arc<Registry>, bytes: usize) -> Result<Self> {
        let mut state = registry.state.lock().expect("record schema registry poisoned");
        let available = REGISTRY_BYTES.saturating_sub(state.bytes).saturating_sub(state.constructing_bytes);
        if state.definitions.len() + state.constructing >= DEFINITIONS || bytes > available {
            return Err(limit());
        }
        state.constructing_bytes += bytes;
        state.constructing += 1;
        Ok(Self { registry: Arc::clone(registry), bytes })
    }

    fn transfer(&mut self, state: &mut State, bytes: usize) {
        state.constructing_bytes -= self.bytes;
        state.constructing -= 1;
        state.bytes += bytes;
        self.bytes = 0;
    }
}

impl Drop for Construction {
    fn drop(&mut self) {
        if self.bytes == 0 {
            return;
        }
        let mut state = self.registry.state.lock().expect("record schema registry poisoned");
        state.constructing_bytes -= self.bytes;
        state.constructing -= 1;
    }
}

impl Pending {
    pub(super) fn is_committed(&self) -> bool {
        self.committed
    }

    pub(super) fn commit(mut self) -> Arc<Definition> {
        if !self.committed {
            let mut state = self.registry.state.lock().expect("record schema registry poisoned");
            let entry = state.definitions.get_mut(&self.key).expect("reserved schema remains owned");
            entry.ready = true;
            self.committed = true;
        }
        Arc::clone(&self.definition)
    }
}

impl Drop for Pending {
    fn drop(&mut self) {
        if self.committed {
            return;
        }
        let removed = {
            let mut state = self.registry.state.lock().expect("record schema registry poisoned");
            let Some(entry) = state.definitions.get(&self.key) else {
                return;
            };
            if !Arc::ptr_eq(&entry.definition, &self.definition) {
                return;
            }
            let entry = state.definitions.remove(&self.key).expect("reserved schema exists");
            state.bytes -= entry.bytes;
            entry
        };
        drop(removed);
    }
}

/* === Public Functions === */

/* === Private Functions === */

fn limit() -> Error {
    Error::ResourceExhausted { resource: "record schema registry" }
}

/* === Tests === */
