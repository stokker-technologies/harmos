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
use std::f64::consts::TAU;
use std::num::NonZeroU32;
use std::time::{Duration, UNIX_EPOCH};

// Third Party Imports
use harmos_storage::*;
use harmos_domain::{DefinitionId, Identified, Identifier, Message};

// Local Imports

/* === Constants === */
pub const AMPLITUDE: f64 = 12.0;
pub const SAMPLES_PER_PERIOD: usize = 64;
/* === Macros === */
/* === Type Aliases === */
/* === Structs === */
#[harmos_domain::message]
#[derive(Clone, Debug, PartialEq)]
pub struct Sample {
    pub phase: u32,
    pub ordinal: u64,
    pub value: f64,
}

#[harmos_domain::message]
#[derive(Clone, Debug, PartialEq)]
pub struct Experiment {
    pub id: String,
    pub label: String,
}

/* === Enums === */
#[harmos_domain::catalog]
pub enum Objects {
    Experiment(Experiment),
}
/* === Traits === */
/* === Implementations === */
impl Identified for Experiment {
    fn definition() -> (DefinitionId, u32) {
        (DefinitionId::new("storage.experiment"), 1)
    }
}
impl StoredObject for Experiment {
    type Key = String;
    const RETENTION: ObjectRetention = ObjectRetention::UntilReleased;
    fn key(&self) -> &String {
        &self.id
    }
}
/* === Public Functions === */
pub fn schema() -> SchemaKey {
    SchemaKey {
        id: Identifier::new("storage.waveform").unwrap(),
        version: NonZeroU32::new(1).unwrap(),
        payload_encoding: PayloadEncoding::PostcardV1,
    }
}

pub fn definition() -> harmos_domain::schema::graph::Schema {
    use harmos_domain::schema::Cardinality;
    use harmos_domain::schema::graph::{Field, Kind};
    harmos_domain::schema::graph::Schema {
        root: 0,
        definitions: vec![vec![
            Field {
                name: "phase".into(),
                tag: 1,
                kind: Kind::U32,
                cardinality: Cardinality::Required,
            },
            Field {
                name: "ordinal".into(),
                tag: 2,
                kind: Kind::U64,
                cardinality: Cardinality::Required,
            },
            Field {
                name: "value".into(),
                tag: 3,
                kind: Kind::F64,
                cardinality: Cardinality::Required,
            },
        ]],
    }
}

/// Integer Periods And Half-Open Uniform Samples Give Analytical RMS A / sqrt(2).
pub fn samples(periods: usize) -> Vec<Sample> {
    (0..periods * SAMPLES_PER_PERIOD)
        .flat_map(|ordinal| {
            (0..3).map(move |phase| {
                let angle =
                    TAU * ordinal as f64 / SAMPLES_PER_PERIOD as f64 - TAU * phase as f64 / 3.0;
                Sample {
                    phase,
                    ordinal: ordinal as u64,
                    value: AMPLITUDE * angle.sin(),
                }
            })
        })
        .collect()
}

pub fn inputs(samples: &[Sample]) -> Vec<StreamInput> {
    let schema = schema();
    samples
        .iter()
        .map(|sample| StreamInput {
            record: RecordInput {
                schema: schema.clone(),
                scope: Some(Identifier::new("experiment").unwrap()),
                item: Identifier::new(format!("phase-{}", sample.phase)).unwrap(),
                source: None,
                source_time: Some(UNIX_EPOCH + Duration::from_millis(sample.ordinal)),
                payload: Message::encode_to_vec(sample),
            },
            stream: Identifier::new(format!("phase-{}", sample.phase)).unwrap(),
            sequence: sample.ordinal,
        })
        .collect()
}

pub fn binary(length: usize) -> Vec<u8> {
    (0..length)
        .map(|index| ((index * 37 + 11) % 251) as u8)
        .collect()
}
/* === Private Functions === */
/* === Tests === */
