/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */
/* === Imports === */

// Standard Library Imports
use std::hint::black_box;
use std::num::{NonZeroU32, NonZeroUsize};

// Third Party Imports
use arrow_array::Float64Array;

// Local Imports
use super::fixture;
use harmos_domain::config::SchemaSettings;
use harmos_domain::model::storage::query::Query;
use harmos_domain::schema::{Described, graph::Schema};
use harmos_query::QueryArrow;
use harmos_query::transform::{
    FieldPath, Transform,
    project::{Project, Projection},
};
use harmos_storage::query::{RecordSelection, RowSlice, Source};
use harmos_storage::{Identifier, Message, PayloadEncoding, RecordInput, SchemaKey, StorageCursor};

/* === Structs === */

pub(crate) struct Mapped {
    fixture: fixture::Fixture,
    schema: SchemaKey,
    at: StorageCursor,
}

#[derive(harmos_domain::Message, Clone, Debug, Default)]
struct PlotSample {
    x: f64,
    y: f64,
}

/* === Implementations === */

impl Mapped {
    pub(crate) async fn finish(self) {
        self.fixture.finish().await;
    }
}

/* === Public Functions === */

pub(crate) async fn prepare(rows: usize) -> Mapped {
    let mut seed = 0x1234_5678_u64;
    let payloads = (0..rows)
        .map(|row| {
            seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
            PlotSample {
                x: row as f64,
                y: (seed >> 11) as f64 / (1_u64 << 53) as f64,
            }
        })
        .collect::<Vec<_>>();

    let mut settings = fixture::config();
    settings.response.batch_rows = NonZeroUsize::new(16_384).unwrap();
    settings.response.batch_bytes = NonZeroUsize::new(8 * 1024 * 1024).unwrap();
    let fixture = fixture::Fixture::new(settings).await;
    let storage = &fixture.storage;
    let schema = SchemaKey {
        id: Identifier::new("plot-sample").unwrap(),
        version: NonZeroU32::MIN,
        payload_encoding: PayloadEncoding::PostcardV1,
    };
    let graph = Schema::from_declaration(PlotSample::DEFINITION, &SchemaSettings::default()).unwrap();
    storage.write_schema(schema.clone(), graph).await.unwrap();
    for (chunk, payloads) in payloads.chunks(4096).enumerate() {
        let records = payloads
            .iter()
            .enumerate()
            .map(|(index, payload)| RecordInput {
                schema: schema.clone(),
                scope: None,
                item: Identifier::new(format!("row-{}", chunk * 4096 + index)).unwrap(),
                source: None,
                source_time: None,
                payload: payload.encode(),
            })
            .collect();
        storage.append_records(records).await.unwrap();
    }
    let at = storage.flush().await.unwrap();
    Mapped { fixture, schema, at }
}

pub(crate) async fn scan(mapped: &Mapped, offset: usize, limit: usize) -> Vec<(f64, f64)> {
    let mut query = Query {
        merge: None,
        at: mapped.at.clone(),
        source: Source::Records(RecordSelection {
            schema: mapped.schema.clone(),
            scope: None,
            item: None,
            source_start: None,
            source_end: None,
        }),
        transforms: vec![],
        slice: RowSlice { offset: 0, limit: None },
    };
    query.transforms.push(Transform::Project(Project {
        fields: vec![
            Projection {
                field: FieldPath::from("payload.x"),
                name: "x".into(),
            },
            Projection {
                field: FieldPath::from("payload.y"),
                name: "y".into(),
            },
        ],
    }));
    query.slice = RowSlice {
        offset: offset as u64,
        limit: Some(limit as u64),
    };
    let mut reader = mapped.fixture.storage.query(query).await.unwrap();
    let mut actual = Vec::new();
    while let Some(output) = reader.next().await.unwrap() {
        let QueryArrow::Batch(batch) = output else { continue };
        let batch = batch.as_record_batch();
        let x = batch.column(0).as_any().downcast_ref::<Float64Array>().unwrap();
        let y = batch.column(1).as_any().downcast_ref::<Float64Array>().unwrap();
        let points = x.values().iter().copied().zip(y.values().iter().copied());
        actual.extend(points);
    }
    black_box(reader.completion().unwrap());
    actual
}
