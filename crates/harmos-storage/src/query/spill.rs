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
// A Million Two-Column Samples Fit Without IPC; Larger Inputs Spill As Before.
const CACHE_BYTES: usize = WORKSPACE_BYTES / 4;

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

#[derive(Clone)]
pub(super) struct Spool {
    file: Option<Arc<NamedTempFile>>,
    batches: Arc<Vec<RecordBatch>>,
    pub schema: Arc<Schema>,
    pub rows: usize,

    #[cfg(test)]
    peak_page_bytes: usize,

    #[cfg(test)]
    peak_page_rows: usize,
}

pub(super) struct Builder {
    spool: Spool,
    writer: Option<StreamWriter<File>>,
    bytes: usize,
    cache_bytes: usize,
}

// Selected Rows Wait As Source Indices; One Interleave Writes Each Output Page.
pub(super) struct Rows {
    builder: Builder,
    sources: Vec<(RecordBatch, usize)>,
    indices: Vec<(usize, usize)>,
    bytes: usize,
    generation: u64,
}

pub(super) struct Cursor {
    pages: Pages,
    pub batch: Option<RecordBatch>,
    pub row: usize,
    source: Option<(u64, usize)>,
}

pub(super) struct Pages {
    // Keep The Private File Alive Until The Last Decoded Page Is Released.
    _spool: Spool,
    reader: Reader,
}

/* === Enums === */

pub(super) enum Reader {
    Memory { batches: Arc<Vec<RecordBatch>>, next: usize },
    Disk(Box<StreamReader<File>>),
}

/* === Traits === */

/* === Implementations === */

impl Builder {
    pub fn new(schema: Arc<Schema>) -> Result<Self> {
        Self::with_cache(schema, 0)
    }

    pub fn cached(schema: Arc<Schema>) -> Result<Self> {
        Self::with_cache(schema, CACHE_BYTES)
    }

    fn with_cache(schema: Arc<Schema>, cache_bytes: usize) -> Result<Self> {
        Ok(Self {
            spool: Spool {
                file: None,
                batches: Arc::new(Vec::new()),
                schema,
                rows: 0,
                #[cfg(test)]
                peak_page_bytes: 0,
                #[cfg(test)]
                peak_page_rows: 0,
            },
            writer: None,
            bytes: 0,
            cache_bytes,
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
            self.store(page, check)?;
            self.spool.rows = self.spool.rows.checked_add(count).ok_or_else(|| invalid("spill row count overflow"))?;
            offset += count;
        }
        Ok(())
    }

    pub fn finish(mut self) -> Result<Spool> {
        if let Some(writer) = &mut self.writer {
            writer.finish().map_err(|_| invalid("spill termination failed"))?;
        }
        drop(self.writer);
        Ok(self.spool)
    }

    fn store(&mut self, page: RecordBatch, check: &mut impl FnMut() -> Result<()>) -> Result<()> {
        let bytes = page.get_array_memory_size();
        if self.writer.is_none() && bytes <= self.cache_bytes.saturating_sub(self.bytes) {
            self.bytes += bytes;
            Arc::get_mut(&mut self.spool.batches).expect("unpublished spool").push(page);
            return Ok(());
        }
        if self.writer.is_none() {
            let file = NamedTempFile::new().map_err(io)?;
            let mut writer = StreamWriter::try_new(file.reopen().map_err(io)?, &self.spool.schema)
                .map_err(|_| invalid("spill schema encoding failed"))?;
            for batch in Arc::get_mut(&mut self.spool.batches).expect("unpublished spool").drain(..) {
                check()?;
                writer.write(&batch).map_err(|_| invalid("spill page encoding failed"))?;
            }
            self.spool.file = Some(Arc::new(file));
            self.writer = Some(writer);
            self.bytes = 0;
        }
        self.writer.as_mut().unwrap().write(&page).map_err(|_| invalid("spill page encoding failed"))
    }
}

impl Spool {
    pub fn reader(&self) -> Result<Reader> {
        let Some(file) = &self.file else {
            return Ok(Reader::Memory { batches: self.batches.clone(), next: 0 });
        };
        let reader = StreamReader::try_new(file.reopen().map_err(io)?, None).map_err(|_| invalid("spill schema decoding failed"))?;
        Ok(Reader::Disk(Box::new(reader)))
    }

    pub fn pages(self) -> Result<Pages> {
        let reader = self.reader()?;
        Ok(Pages { _spool: self, reader })
    }
}

impl Iterator for Reader {
    type Item = Result<RecordBatch>;

    fn next(&mut self) -> Option<Self::Item> {
        match self {
            Self::Memory { batches, next } => {
                let batch = batches.get(*next)?.clone();
                *next += 1;
                Some(Ok(batch))
            }
            Self::Disk(reader) => reader.next().map(|batch| batch.map_err(|_| invalid("spill page decoding failed"))),
        }
    }
}

impl Rows {
    pub fn new(schema: Arc<Schema>) -> Result<Self> {
        Ok(Self {
            builder: Builder::new(schema)?,
            sources: Vec::new(),
            indices: Vec::new(),
            bytes: 0,
            generation: 0,
        })
    }

    pub fn push(&mut self, cursor: &mut Cursor, check: &mut impl FnMut() -> Result<()>) -> Result<()> {
        let batch = cursor.batch.as_ref().ok_or_else(|| invalid("spill row cursor is exhausted"))?;
        let registered = cursor.source.filter(|(generation, _)| *generation == self.generation).map(|(_, source)| source);
        let row_bytes = match registered {
            Some(source) => self.sources[source].1,
            None => batch.get_array_memory_size().div_ceil(batch.num_rows().max(1)),
        };
        if !self.indices.is_empty() && (self.bytes.saturating_add(row_bytes) > PAGE_BYTES || self.indices.len() == 1024) {
            self.flush(check)?;
        }

        // A Flush Retires Every Registered Source; The Cursor's Batch Registers Again.
        let source = match cursor.source.filter(|(generation, _)| *generation == self.generation) {
            Some((_, source)) => source,
            None => {
                self.sources.push((batch.clone(), row_bytes));
                cursor.source = Some((self.generation, self.sources.len() - 1));
                self.sources.len() - 1
            }
        };
        self.bytes = self.bytes.checked_add(row_bytes).ok_or_else(|| invalid("spill row size overflow"))?;
        self.indices.push((source, cursor.row));
        Ok(())
    }

    fn flush(&mut self, check: &mut impl FnMut() -> Result<()>) -> Result<()> {
        if self.indices.is_empty() {
            return Ok(());
        }
        let sources = self.sources.iter().map(|(batch, _)| batch).collect::<Vec<_>>();
        let batch = arrow_select::interleave::interleave_record_batch(&sources, &self.indices)
            .map_err(|_| invalid("spill output page construction failed"))?;
        self.sources.clear();
        self.indices.clear();
        self.bytes = 0;
        self.generation = self.generation.checked_add(1).ok_or_else(|| invalid("spill page generation overflow"))?;
        self.builder.push(&batch, check)
    }

    pub fn finish(mut self, check: &mut impl FnMut() -> Result<()>) -> Result<Spool> {
        self.flush(check)?;
        self.builder.finish()
    }
}

impl Cursor {
    pub fn new(spool: Spool) -> Result<Self> {
        let mut cursor = Self {
            pages: spool.pages()?,
            batch: None,
            row: 0,
            source: None,
        };
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
        self.source = None;
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
        let path = spool.file.as_ref().unwrap().path().to_owned();
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

    #[test]
    fn cached_pages_share_arrays_and_spill_exactly_at_capacity() {
        let schema = Arc::new(Schema::new(vec![Field::new("value", DataType::UInt64, false)]));
        let array = UInt64Array::from((0..19).collect::<Vec<u64>>());
        let batch = RecordBatch::try_new(schema.clone(), vec![Arc::new(array)]).unwrap();
        let mut memory = Builder::cached(schema.clone()).unwrap();
        memory.push(&batch, &mut || Ok(())).unwrap();
        let memory = memory.finish().unwrap();
        assert!(memory.file.is_none());
        let first = memory.reader().unwrap().next().unwrap().unwrap();
        let second = memory.reader().unwrap().next().unwrap().unwrap();
        assert!(Arc::ptr_eq(first.column(0), second.column(0)));

        let bytes = first.get_array_memory_size();
        let mut builder = Builder::with_cache(schema, bytes).unwrap();
        builder.push(&batch, &mut || Ok(())).unwrap();
        assert!(builder.spool.file.is_none());
        builder.push(&batch, &mut || Ok(())).unwrap();
        assert!(builder.spool.batches.is_empty());
        let spool = builder.finish().unwrap();
        let path = spool.file.as_ref().unwrap().path().to_owned();
        let expected = arrow_select::concat::concat_batches(&batch.schema(), [&batch, &batch]).unwrap();
        let pages = spool.reader().unwrap().collect::<Result<Vec<_>>>().unwrap();
        let actual = arrow_select::concat::concat_batches(&batch.schema(), &pages).unwrap();
        assert_eq!(actual, expected);
        drop(spool);
        assert!(!path.exists());
    }

    #[test]
    fn cancellation_during_cache_spill_preserves_error_and_cleans_file() {
        let schema = Arc::new(Schema::new(vec![Field::new("value", DataType::UInt64, false)]));
        let batch = RecordBatch::try_new(schema.clone(), vec![Arc::new(UInt64Array::from(vec![7; 100]))]).unwrap();
        let mut builder = Builder::with_cache(schema, batch.get_array_memory_size()).unwrap();
        builder.push(&batch, &mut || Ok(())).unwrap();
        let mut calls = 0;
        let result = builder.push(&batch, &mut || {
            calls += 1;
            if calls == 2 { Err(Error::Closing { domain: "cancel spill" }) } else { Ok(()) }
        });
        assert!(matches!(result, Err(Error::Closing { .. })));
        assert!(builder.spool.file.is_none());
    }
}
