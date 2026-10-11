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
use std::time::UNIX_EPOCH;

// Third Party Imports

// Local Imports
use super::*;
use harmos::storage::frame::*;
use harmos::storage::reduce::ScalarValue;

/* === Constants === */
const TOKEN_BYTES: usize = std::mem::size_of::<u64>();

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */
pub struct SortedScan {
    snapshot: Vec<u8>,
    scan: Scan,
    order: ScanOrder,
    rows: Vec<ProjectedRow>,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */
impl<C: ObjectCatalog> Data<C> {
    pub(super) fn frame_scan<'a>(
        &'a self,
        view: &'a ReadView,
        scan: &'a Scan,
        order: &'a ScanOrder,
        cursor: Option<&'a ScanCursor>,
        limits: ReadLimits,
    ) -> StorageFuture<'a, ScanPage> {
        Box::pin(async move {
            view.check_kind(scan.selection.kind, &scan.selection, None)?;
            let offset = match cursor {
                None => 0,
                Some(cursor) => {
                    u64::from_le_bytes(cursor.token.as_slice().try_into().map_err(|_| {
                        Error::InvalidInput {
                            field: "test frame cursor",
                            reason: "foreign continuation",
                        }
                    })?) as usize
                }
            };
            let active = view.retain_computation()?.ok_or(Error::ViewExpired)?;
            self.control.frame_pages.fetch_add(1, Ordering::SeqCst);
            if self.control.block_read.load(Ordering::SeqCst) {
                self.control.read_entered.notify_one();
                self.control.read_release.notified().await;
            }

            let held = self.snapshots.lock().unwrap();
            let token = &view.snapshot().token;
            let snapshot = held.get(token).ok_or(Error::ViewExpired)?;
            let execution = view.execution_limits().ok_or(Error::ViewExpired)?;
            let remaining = execution
                .memory_bytes
                .get()
                .checked_sub(snapshot.query_bytes)
                .and_then(NonZeroUsize::new)
                .ok_or(Error::ResourceExhausted {
                    resource: "test frame memory",
                })?;
            let schema = snapshot
                .schemas
                .values()
                .find(|schema| Some(&schema.key) == scan.selection.schema.as_ref())
                .ok_or(Error::InvalidInput {
                    field: "test frame schema",
                    reason: "selected schema absent from original saved view",
                })?;
            let projection =
                ScalarProjection::new(&schema.fields, scan.columns.clone(), remaining)?;
            let selection = &scan.selection;
            let candidates = snapshot
                .records
                .values()
                .map(|record| (record, None))
                .filter(|_| selection.kind == ReadKind::Records)
                .chain(
                    snapshot
                        .rows
                        .values()
                        .map(|row| (&row.record, Some(&row.stream)))
                        .filter(|_| selection.kind == ReadKind::Stream),
                )
                .filter(|(record, stream)| {
                    record.position.sequence > selection.after
                        && record.position.sequence
                            <= selection.through.unwrap_or(view.saved().sequence)
                        && matches_record(record, selection)
                        && selection
                            .stream
                            .as_ref()
                            .is_none_or(|selected| *stream == Some(selected))
                });
            let decode = |record: &StoredRecord, stream: Option<&Identifier>| {
                active.check()?;
                self.control.frame_decodes.fetch_add(1, Ordering::SeqCst);
                let values = projection.project(
                    &record.payload,
                    |path, allowance| metadata(record, stream, path, allowance),
                    limits.bytes,
                )?;
                Ok::<_, Error>(ProjectedRow {
                    capture_order: record.position.sequence,
                    values,
                })
            };

            // Capture Order Decodes Only This Page; Keyed Order Sorts The Window Once.
            let mut rows = Vec::new();
            let mut bytes = TOKEN_BYTES;
            let mut more = false;
            let mut push = |row: ProjectedRow, rows: &mut Vec<ProjectedRow>| {
                let text = text_bytes(&row);
                if rows.len() == limits.rows.get() || bytes + text > limits.bytes.get() {
                    return Ok::<_, Error>(false);
                }
                bytes += text;
                rows.push(row);
                Ok(true)
            };
            match order {
                ScanOrder::Capture => {
                    for (record, stream) in candidates.skip(offset) {
                        if rows.len() == limits.rows.get() {
                            more = true;
                            break;
                        }
                        let row = decode(record, stream)?;
                        if !push(row, &mut rows)? {
                            more = true;
                            break;
                        }
                    }
                }
                ScanOrder::Keyed { .. } => {
                    let mut sorted = self.control.frame_sorted.lock().unwrap();
                    let reuse = offset > 0
                        && sorted.as_ref().is_some_and(|sorted| {
                            sorted.snapshot == *token
                                && sorted.scan == *scan
                                && sorted.order == *order
                        });
                    if !reuse {
                        let mut window = Vec::new();
                        for (record, stream) in candidates {
                            window.push(decode(record, stream)?);
                        }
                        let mut failure = None;
                        window.sort_by(|a, b| {
                            order.compare(a, b).unwrap_or_else(|error| {
                                failure.get_or_insert(error);
                                std::cmp::Ordering::Equal
                            })
                        });
                        if let Some(error) = failure {
                            return Err(error);
                        }
                        *sorted = Some(SortedScan {
                            snapshot: token.clone(),
                            scan: scan.clone(),
                            order: order.clone(),
                            rows: window,
                        });
                    }
                    let window = &sorted.as_ref().expect("sorted window").rows;
                    for row in window.iter().skip(offset) {
                        if !push(row.clone(), &mut rows)? {
                            more = true;
                            break;
                        }
                    }
                }
            }
            if rows.is_empty() && more {
                return Err(Error::ResourceExhausted {
                    resource: "test frame row text",
                });
            }
            let next = more.then(|| ScanCursor {
                token: ((offset + rows.len()) as u64).to_le_bytes().to_vec(),
            });
            Ok(ScanPage { rows, next })
        })
    }
}

/* === Public Functions === */

/* === Private Functions === */
fn metadata(
    record: &StoredRecord,
    stream: Option<&Identifier>,
    path: &ScalarPath,
    allowance: NonZeroUsize,
) -> Result<ScalarValue> {
    Ok(match path {
        ScalarPath::SourceTime => record
            .source_time
            .map(|time| {
                ScalarValue::Datetime(match time.duration_since(UNIX_EPOCH) {
                    Ok(time) => time.as_nanos() as i128,
                    Err(error) => -(error.duration().as_nanos() as i128),
                })
            })
            .unwrap_or(ScalarValue::Missing),
        ScalarPath::CaptureOrder => ScalarValue::Unsigned(record.position.sequence),
        ScalarPath::Scope => text(record.scope.as_ref(), allowance)?,
        ScalarPath::Stream => text(stream, allowance)?,
        ScalarPath::Item => text(Some(&record.item), allowance)?,
        ScalarPath::Payload(_) => unreachable!("payload fields are visited, not supplied"),
    })
}
fn text(value: Option<&Identifier>, allowance: NonZeroUsize) -> Result<ScalarValue> {
    match value {
        Some(value) if value.memory_size() <= allowance.get() => {
            Ok(ScalarValue::Text(value.to_string()))
        }
        Some(_) => Err(Error::ResourceExhausted {
            resource: "test frame metadata",
        }),
        None => Ok(ScalarValue::Missing),
    }
}
fn text_bytes(row: &ProjectedRow) -> usize {
    row.values
        .iter()
        .map(|value| match value {
            ScalarValue::Text(text) => text.capacity(),
            _ => 0,
        })
        .sum()
}

/* === Tests === */
