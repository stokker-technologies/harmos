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
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::value::RawValue;
// Local Imports
use crate::evidence::EvidenceError;
use crate::{DefinitionId, Identified, state::At, state::FactSource, state::Position, state::SystemFact};

/* === Constants === */
pub(crate) const SOURCE: &str = "artifact";

pub(crate) const FINALIZED: &str = "finalized";

pub(crate) const VERSION: u32 = 1;

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EvidenceRecord {
    pub kind: DefinitionId,
    pub version: u32,
    pub scope: String,
    pub key: String,
    pub length: u64,
    pub finalized: Option<At<u64>>,
    described: Vec<u8>,
}

#[derive(Serialize, Deserialize)]
struct Recorded {
    kind: String,
    version: u32,
    scope: String,
    key: String,
    digest: u64,
    length: u64,
    #[serde(default = "nothing")]
    details: Box<RawValue>,
}

/* === Enums === */

/* === Traits === */
pub trait EvidenceKind: Identified {
    type Details: Serialize + DeserializeOwned;
}

/* === Implementations === */
impl EvidenceRecord {
    pub fn details<K: EvidenceKind>(&self) -> Result<K::Details, EvidenceError> {
        let (named, _) = K::definition();
        if named != self.kind {
            return Err(EvidenceError::Mismatched { kind: self.kind.clone(), named });
        }

        serde_json::from_slice(&self.described)
            .map_err(|error| EvidenceError::Malformed { kind: self.kind.clone(), cause: error.to_string() })
    }

    pub(crate) fn new(kind: DefinitionId, version: u32, scope: String, key: String, length: u64, described: Vec<u8>) -> Self {
        Self { kind, version, scope, key, length, finalized: None, described }
    }

    pub(crate) fn folded(fact: &SystemFact, position: Position) -> Option<Self> {
        let FactSource::Work(source) = &fact.source else {
            return None;
        };
        if source != SOURCE || fact.name != FINALIZED {
            return None;
        }
        let recorded: Recorded = serde_json::from_slice(&fact.payload).ok()?;

        Some(Self {
            kind: DefinitionId::new(recorded.kind),
            version: recorded.version,
            scope: recorded.scope,
            key: recorded.key,
            length: recorded.length,
            finalized: Some(At { value: recorded.digest, position }),
            described: recorded.details.get().as_bytes().to_vec(),
        })
    }

    pub(crate) fn recorded(&self, digest: u64) -> SystemFact {
        let payload = serde_json::to_vec(&Recorded {
            kind: self.kind.as_str().to_owned(),
            version: self.version,
            scope: self.scope.clone(),
            key: self.key.clone(),
            digest,
            length: self.length,
            details: described(&self.described),
        })
        .expect("the fixed evidence fact shape serializes");

        SystemFact { source: FactSource::Work(SOURCE.to_owned()), name: FINALIZED.to_owned(), version: VERSION, payload }
    }
}

/* === Public Functions === */

/* === Private Functions === */
fn nothing() -> Box<RawValue> {
    described(b"null")
}

fn described(details: &[u8]) -> Box<RawValue> {
    let text = String::from_utf8(details.to_vec()).expect("a serialized payload is JSON text");

    RawValue::from_string(text).expect("a serialized payload is JSON")
}

/* === Tests === */
