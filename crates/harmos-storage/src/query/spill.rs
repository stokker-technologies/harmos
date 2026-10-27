/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Private Paged Arrow Spill Ownership

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::fs::File;
use std::sync::Arc;

// Third Party Imports
use arrow_array::RecordBatch;
use arrow_ipc::{reader::StreamReader, writer::StreamWriter};
use arrow_schema::Schema;
use tempfile::NamedTempFile;

// Local Imports
use super::transform::invalid;
use crate::{BackendFailure, Error, Result};

/* === Constants === */

pub(super) const PAGE_ROWS: usize = 16_384;
// Leave Room For Two Merge Inputs, Compact Output, IPC Buffers And Conversions.
const WORKSPACE_BYTES: usize = 128 * 1024 * 1024;
pub(super) const PAGE_BYTES: usize = WORKSPACE_BYTES / 16;

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

pub(super) struct Spool {
    file: NamedTempFile,
    pub schema: Arc<Schema>,
    pub rows: usize,

    #[cfg(test)]
    peak_page_bytes: usize,

    #[cfg(test)]
    peak_page_rows: usize,
}

pub(super) struct Builder {
    spool: Spool,
    writer: StreamWriter<File>,
}

pub(super) struct Rows {
    builder: Builder,
    pending: Vec<RecordBatch>,
    bytes: usize,
    count: usize,
}

pub(super) struct Cursor {
    pages: Pages,
    pub batch: Option<RecordBatch>,
    pub row: usize,
}

pub(super) struct Pages {
    // Keep The Private File Alive Until The Last Decoded Page Is Released.
    _spool: Spool,
    reader: StreamReader<File>,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl Builder {
    pub fn new(schema: Arc<Schema>) -> Result<Self> {
        let file = NamedTempFile::new().map_err(io)?;
        let writer = StreamWriter::try_new(file.reopen().map_err(io)?, &schema).map_err(|_| invalid("spill schema encoding failed"))?;
        Ok(Self {
            spool: Spool {
                file,
                schema,
                rows: 0,
                #[cfg(test)]
                peak_page_bytes: 0,
                #[cfg(test)]
                peak_page_rows: 0,
            },
            writer,
        })
    }

    pub fn push(&mut self, batch: &RecordBatch, check: &mut impl FnMut() -> Result<()>) -> Result<()> {
        let mut offset = 0;
        while offset < batch.num_rows() {
            check()?;
            let mut count = PAGE_ROWS.min(batch.num_rows() - offset);
            // Compact Slices Before Measuring: A Slice Can Retain Its Entire Parent.
            let page = loop {
                let page = super::transform::take(batch, (offset..offset + count).map(|row| row as u64).collect())?;
                if page.get_array_memory_size() <= PAGE_BYTES || count == 1 {
                    break page;
                }
                count = count.div_ceil(2);
            };
            #[cfg(test)]
            {
                self.spool.peak_page_bytes = self.spool.peak_page_bytes.max(page.get_array_memory_size());
                self.spool.peak_page_rows = self.spool.peak_page_rows.max(page.num_rows());
            }
            self.writer.write(&page).map_err(|_| invalid("spill page encoding failed"))?;
            self.spool.rows = self.spool.rows.checked_add(count).ok_or_else(|| invalid("spill row count overflow"))?;
            offset += count;
        }
        Ok(())
    }

    pub fn finish(mut self) -> Result<Spool> {
        self.writer.finish().map_err(|_| invalid("spill termination failed"))?;
        drop(self.writer);
        Ok(self.spool)
    }
}

impl Spool {
    pub fn reader(&self) -> Result<StreamReader<File>> {
        StreamReader::try_new(self.file.reopen().map_err(io)?, None).map_err(|_| invalid("spill schema decoding failed"))
    }

    pub fn pages(self) -> Result<Pages> {
        let reader = self.reader()?;
        Ok(Pages { _spool: self, reader })
    }
}

impl Rows {
    pub fn new(schema: Arc<Schema>) -> Result<Self> {
        Ok(Self { builder: Builder::new(schema)?, pending: Vec::new(), bytes: 0, count: 0 })
    }

    pub fn push(&mut self, batch: &RecordBatch, row: usize, check: &mut impl FnMut() -> Result<()>) -> Result<()> {
        let row = super::transform::take(batch, vec![row as u64])?;
        let bytes = row.get_array_memory_size();
        if self.count != 0 && (self.bytes.saturating_add(bytes) > PAGE_BYTES || self.count == 1024) {
            self.flush(check)?;
        }
        self.bytes = self.bytes.checked_add(bytes).ok_or_else(|| invalid("spill row size overflow"))?;
        self.count += 1;
        self.pending.push(row);
        Ok(())
    }

    fn flush(&mut self, check: &mut impl FnMut() -> Result<()>) -> Result<()> {
        if self.pending.is_empty() {
            return Ok(());
        }
        let batch = arrow_select::concat::concat_batches(&self.builder.spool.schema, self.pending.iter())
            .map_err(|_| invalid("spill output page construction failed"))?;
        self.pending.clear();
        self.count = 0;
        self.bytes = 0;
        self.builder.push(&batch, check)
    }

    pub fn finish(mut self, check: &mut impl FnMut() -> Result<()>) -> Result<Spool> {
        self.flush(check)?;
        self.builder.finish()
    }
}

impl Cursor {
    pub fn new(spool: Spool) -> Result<Self> {
        let mut cursor = Self { pages: spool.pages()?, batch: None, row: 0 };
        cursor.advance()?;
        Ok(cursor)
    }

    pub fn advance(&mut self) -> Result<()> {
        if let Some(batch) = &self.batch {
            self.row += 1;
            if self.row < batch.num_rows() {
                return Ok(());
            }
        }
        loop {
            self.batch = self.pages.next()?;
            self.row = 0;
            if self.batch.as_ref().is_none_or(|batch| batch.num_rows() != 0) {
                return Ok(());
            }
        }
    }
}

impl Pages {
    pub fn next(&mut self) -> Result<Option<RecordBatch>> {
        self.reader.next().transpose().map_err(|_| invalid("spill page decoding failed"))
    }
}

/* === Public Functions === */

/* === Private Functions === */

pub(super) fn io(error: std::io::Error) -> Error {
    Error::backend("query spill I/O", BackendFailure::Transient, error)
}

/* === Tests === */

#[cfg(test)]
mod tests {
    use super::*;
    use arrow_array::UInt64Array;
    use arrow_schema::{DataType, Field};

    #[test]
    fn seven_day_volume_keeps_only_bounded_pages_and_releases_spill() {
        let schema = Arc::new(Schema::new(vec![Field::new("value", DataType::UInt64, false)]));
        let mut builder = Builder::new(schema.clone()).unwrap();
        let page = RecordBatch::try_new(schema, vec![Arc::new(UInt64Array::from((0..20_000).collect::<Vec<u64>>()))]).unwrap();
        for _hour in 0..7 * 24 {
            builder.push(&page, &mut || Ok(())).unwrap();
        }
        let spool = builder.finish().unwrap();
        assert_eq!(spool.rows, 7 * 24 * 20_000);
        assert!(spool.peak_page_bytes <= PAGE_BYTES);
        assert!(spool.peak_page_rows <= PAGE_ROWS);
        let path = spool.file.path().to_owned();
        let mut pages = spool.pages().unwrap();
        let mut rows = 0;
        while let Some(page) = pages.next().unwrap() {
            assert!(page.num_rows() <= PAGE_ROWS);
            assert!(page.get_array_memory_size() <= PAGE_BYTES);
            rows += page.num_rows();
        }
        assert_eq!(rows, 7 * 24 * 20_000);
        drop(pages);
        assert!(!path.exists());
    }
}
