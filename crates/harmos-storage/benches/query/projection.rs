/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */
/* === Imports === */

use super::mapping_fixture::*;

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

    let mut settings = storage_settings();
    settings.response.batch_rows = NonZeroUsize::new(16_384).unwrap();
    settings.response.batch_bytes = NonZeroUsize::new(8 * 1024 * 1024).unwrap();
    Mapped::new(settings, "plot-sample", &payloads).await
}

pub(crate) async fn scan(mapped: &Mapped, offset: usize, limit: usize) -> Vec<(f64, f64)> {
    use harmos_storage::query::transform::{
        FieldPath, Transform,
        project::{Project, Projection},
    };
    let mut query = mapped.query(None);
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
    let mut reader = mapped.storage.query(query).await.unwrap();
    let mut actual = Vec::new();
    while let Some(output) = reader.next().await.unwrap() {
        let QueryArrow::Batch(batch) = output else { continue };
        let batch = batch.as_record_batch();
        let x = batch.column(0).as_any().downcast_ref::<Float64Array>().unwrap();
        let y = batch.column(1).as_any().downcast_ref::<Float64Array>().unwrap();
        let points = x.values().iter().copied().zip(y.values().iter().copied());
        actual.extend(points);
    }
    std::hint::black_box(reader.completion().unwrap());
    actual
}
