/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Shared Bounded Transfer Contracts For Carrier Benchmarks.

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

#[cfg(feature = "runtime")]
use {
    harmos::MemorySize,
    std::collections::{BTreeMap, VecDeque},
    std::sync::Mutex,
};

// Standard Library Imports

// Third Party Imports

// Local Imports

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

#[harmos::message]
#[derive(Clone)]
pub struct Record {
    pub sequence: u64,
    pub payload: Vec<u8>,
}

#[harmos::message]
#[derive(Clone)]
pub struct Sample {
    pub sequence: u64,
    pub timestamp: u64,
    pub value: f64,
}

#[harmos::message]
#[derive(Clone)]
pub struct Batch {
    pub records: Vec<Record>,
    pub samples: Vec<Sample>,
    pub bytes: Vec<u8>,
}

#[harmos::message]
#[derive(Clone)]
pub struct Receipt {
    pub records: u64,
    pub samples: u64,
    pub bytes: u64,
}

#[harmos::operation(resources = [crate::transfer::Store],
    id = "benchmark_upload", response = Receipt, input = Batch,
    policy = crate::POLICY,
    visibility = harmos::operations::Visibility::Shared,
    replay = harmos::operations::ReplaySafety::Idempotent,
)]
pub struct Upload {
    pub transfer: u64,
}

#[harmos::operation(resources = [crate::transfer::Store],
    id = "benchmark_download", response = Receipt, output = Batch,
    policy = crate::POLICY,
    visibility = harmos::operations::Visibility::Shared,
    replay = harmos::operations::ReplaySafety::Unknown,
)]
pub struct Download {
    pub transfer: u64,
}

#[harmos::operation(resources = [crate::transfer::Store],
    id = "benchmark_put", response = Receipt,
    policy = crate::POLICY,
    visibility = harmos::operations::Visibility::Shared,
    replay = harmos::operations::ReplaySafety::Unknown,
)]
pub struct Put {
    pub transfer: u64,
    pub first: bool,
    pub batch: Batch,
}

#[harmos::operation(resources = [crate::transfer::Store],
    id = "benchmark_take", response = Batch,
    policy = crate::POLICY,
    visibility = harmos::operations::Visibility::Shared,
    replay = harmos::operations::ReplaySafety::Unknown,
)]
pub struct Take {
    pub transfer: u64,
}

#[harmos::operation(operations = [harmos::builtins::storage::HashFile],
    id = "benchmark_hash_file", response = harmos::domain::model::storage::FileHash,
    policy = crate::POLICY,
    visibility = harmos::operations::Visibility::Shared,
    replay = harmos::operations::ReplaySafety::Idempotent,
)]
pub struct Hash {
    pub request: <harmos::builtins::storage::HashFile as harmos::OperationDeclaration>::Request,
}

#[cfg(feature = "runtime")]
#[derive(Default)]
pub struct Store {
    batches: Mutex<BTreeMap<u64, Stored>>,
}

#[cfg(feature = "runtime")]
#[derive(Default)]
struct Stored {
    bytes: usize,
    batches: VecDeque<Batch>,
}

/* === Enums === */

#[harmos::catalog]
pub enum Transfers {
    Upload(Upload),
    Download(Download),
    Put(Put),
    Take(Take),
    Hash(Hash),
}

#[harmos::catalog]
pub enum Guest {
    #[nested]
    Shared(crate::Operations),

    #[nested]
    Transfers(Transfers),
}

#[harmos::catalog]
pub enum Native {
    #[nested]
    Shared(crate::Native),

    #[nested]
    Transfers(Transfers),
}

/* === Traits === */

/* === Implementations === */

#[cfg(feature = "runtime")]
impl harmos::Operation for Upload {
    type Error = crate::Error;

    async fn run(
        mut self, ctx: harmos::Context<'_, Self>,
    ) -> Result<harmos::Response<<Self as harmos::OperationDeclaration>::Response>, Self::Error> {
        let store = ctx.resource::<crate::transfer::Store>();
        let request = &mut self;

        let mut batches = VecDeque::new();
        let mut retained = 0usize;
        let mut receipt = Receipt { records: 0, samples: 0, bytes: 0 };
        while let Some(item) = ctx.streams().recv().await {
            let (batch, _claims) = item.into_owned_request()?;
            receipt.records += batch.records.len() as u64;
            receipt.samples += batch.samples.len() as u64;
            receipt.bytes += batch.bytes.len() as u64;
            retained = retained.saturating_add(batch.memory_size());
            if retained > 64 * 1024 * 1024 {
                return Err(crate::Error::Stream(harmos::operations::StreamError::Capacity));
            }
            batches.push_back(batch);
        }
        let mut stores = store.batches.lock().unwrap();
        let other_bytes: usize = stores.iter().filter(|(id, _)| **id != request.transfer).map(|(_, stored)| stored.bytes).sum();
        if retained.saturating_add(other_bytes) > 64 * 1024 * 1024 {
            return Err(crate::Error::Stream(harmos::operations::StreamError::Capacity));
        }
        stores.insert(request.transfer, Stored { bytes: retained, batches });
        Ok(receipt.into())
    }
}

#[cfg(feature = "runtime")]
impl harmos::Operation for Download {
    type Error = crate::Error;

    async fn run(
        mut self, ctx: harmos::Context<'_, Self>,
    ) -> Result<harmos::Response<<Self as harmos::OperationDeclaration>::Response>, Self::Error> {
        let store = ctx.resource::<crate::transfer::Store>();
        let request = &mut self;

        let batches = store.batches.lock().unwrap().remove(&request.transfer).unwrap_or_default();
        let mut receipt = Receipt { records: 0, samples: 0, bytes: 0 };
        for batch in batches.batches {
            receipt.records += batch.records.len() as u64;
            receipt.samples += batch.samples.len() as u64;
            receipt.bytes += batch.bytes.len() as u64;
            let bytes = batch.memory_size();
            ctx.streams().send_with(bytes, || Ok(batch)).await?;
        }
        Ok(receipt.into())
    }
}

#[cfg(feature = "runtime")]
impl harmos::Operation for Put {
    type Error = crate::Error;

    async fn run(
        mut self, ctx: harmos::Context<'_, Self>,
    ) -> Result<harmos::Response<<Self as harmos::OperationDeclaration>::Response>, Self::Error> {
        let store = ctx.resource::<crate::transfer::Store>();
        let request = &mut self;

        let mut stores = store.batches.lock().unwrap();
        if request.first {
            stores.remove(&request.transfer);
        }
        let retained: usize = stores.values().map(|stored| stored.bytes).sum();
        let bytes = retained.saturating_add(request.batch.memory_size());
        if bytes > 64 * 1024 * 1024 {
            return Err(crate::Error::Stream(harmos::operations::StreamError::Capacity));
        }
        let empty = Batch {
            records: Vec::new(),
            samples: Vec::new(),
            bytes: Vec::new(),
        };
        let batch = std::mem::replace(&mut request.batch, empty);
        let receipt = Receipt {
            records: batch.records.len() as u64,
            samples: batch.samples.len() as u64,
            bytes: batch.bytes.len() as u64,
        };
        let stored = stores.entry(request.transfer).or_default();
        stored.bytes += batch.memory_size();
        stored.batches.push_back(batch);
        Ok(receipt.into())
    }
}

#[cfg(feature = "runtime")]
impl harmos::Operation for Take {
    type Error = crate::Error;

    async fn run(
        mut self, ctx: harmos::Context<'_, Self>,
    ) -> Result<harmos::Response<<Self as harmos::OperationDeclaration>::Response>, Self::Error> {
        let store = ctx.resource::<crate::transfer::Store>();
        let request = &mut self;

        let mut stores = store.batches.lock().unwrap();
        let Some(stored) = stores.get_mut(&request.transfer) else {
            return Ok(Batch {
                records: Vec::new(),
                samples: Vec::new(),
                bytes: Vec::new(),
            }
            .into());
        };
        let batch = stored.batches.pop_front().unwrap_or_else(|| Batch {
            records: Vec::new(),
            samples: Vec::new(),
            bytes: Vec::new(),
        });
        stored.bytes = stored.bytes.saturating_sub(batch.memory_size());
        if stored.batches.is_empty() {
            stores.remove(&request.transfer);
        }
        Ok(batch.into())
    }
}

#[cfg(feature = "runtime")]
impl harmos::Operation for Hash {
    type Error = crate::Error;

    async fn run(
        mut self, ctx: harmos::Context<'_, Self>,
    ) -> Result<harmos::Response<<Self as harmos::OperationDeclaration>::Response>, Self::Error> {
        let request = &mut self;

        ctx.operation(request.request.clone()).await.map_err(Into::into)
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
