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

#[harmos::message]
pub struct UploadRequest {
    pub transfer: u64,
}

#[harmos::operation(
    id = "benchmark_upload", request = UploadRequest, infer_request, response = Receipt, input = Batch,
    policy = crate::POLICY,
    visibility = harmos::operations::Visibility::Shared,
    replay = harmos::operations::ReplaySafety::Idempotent,
)]
pub struct Upload {
    #[cfg(feature = "runtime")]
    store: std::sync::Arc<crate::transfer::Store>,
}

#[harmos::message]
pub struct DownloadRequest {
    pub transfer: u64,
}

#[harmos::operation(
    id = "benchmark_download", request = DownloadRequest, infer_request, response = Receipt, output = Batch,
    policy = crate::POLICY,
    visibility = harmos::operations::Visibility::Shared,
    replay = harmos::operations::ReplaySafety::Unknown,
)]
pub struct Download {
    #[cfg(feature = "runtime")]
    store: std::sync::Arc<crate::transfer::Store>,
}

#[harmos::message]
pub struct PutRequest {
    pub transfer: u64,
    pub first: bool,
    pub batch: Batch,
}

#[harmos::operation(
    id = "benchmark_put", request = PutRequest, infer_request, response = Receipt,
    policy = crate::POLICY,
    visibility = harmos::operations::Visibility::Shared,
    replay = harmos::operations::ReplaySafety::Unknown,
)]
pub struct Put {
    #[cfg(feature = "runtime")]
    store: std::sync::Arc<crate::transfer::Store>,
}

#[harmos::message]
pub struct TakeRequest {
    pub transfer: u64,
}

#[harmos::operation(
    id = "benchmark_take", request = TakeRequest, infer_request, response = Batch,
    policy = crate::POLICY,
    visibility = harmos::operations::Visibility::Shared,
    replay = harmos::operations::ReplaySafety::Unknown,
)]
pub struct Take {
    #[cfg(feature = "runtime")]
    store: std::sync::Arc<crate::transfer::Store>,
}

#[harmos::message]
pub struct HashRequest {
    pub request: <harmos::domain::operations::storage::HashFile as harmos::OperationDeclaration>::Request,
}

#[harmos::operation(
    id = "benchmark_hash_file", request = HashRequest, infer_request, response = harmos::domain::model::storage::FileHash,
    policy = crate::POLICY,
    visibility = harmos::operations::Visibility::Shared,
    replay = harmos::operations::ReplaySafety::Idempotent,
)]
pub struct Hash {
    #[cfg(feature = "runtime")]
    provider: std::sync::Arc<crate::resources::HashProvider>,
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
pub enum GuestRequires {
    HashFile(harmos::domain::operations::storage::HashFile),
}

#[harmos::catalog]
pub enum NativeRequires {
    #[nested]
    Shared(crate::Requires),
    HashFile(harmos::domain::operations::storage::HashFile),
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
    type Request = UploadRequest;
    type Response = Receipt;
    type Error = crate::Error;
    type Endpoints = harmos::operations::StreamInput<Batch>;

    async fn initialize(context: harmos::OperationContext) -> Result<Self, Self::Error> {
        Ok(Self { store: context.dependency()? })
    }

    async fn run(
        &mut self, _: harmos::OperationContext, request: &mut Self::Request, endpoints: &mut Self::Endpoints,
    ) -> Result<harmos::Response<Self::Response>, Self::Error> {
        let mut batches = VecDeque::new();
        let mut retained = 0usize;
        let mut receipt = Receipt { records: 0, samples: 0, bytes: 0 };
        while let Some(item) = endpoints.recv().await {
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
        let mut stores = self.store.batches.lock().unwrap();
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
    type Request = DownloadRequest;
    type Response = Receipt;
    type Error = crate::Error;
    type Endpoints = harmos::operations::StreamOutput<Batch>;

    async fn initialize(context: harmos::OperationContext) -> Result<Self, Self::Error> {
        Ok(Self { store: context.dependency()? })
    }

    async fn run(
        &mut self, _: harmos::OperationContext, request: &mut Self::Request, endpoints: &mut Self::Endpoints,
    ) -> Result<harmos::Response<Self::Response>, Self::Error> {
        let batches = self.store.batches.lock().unwrap().remove(&request.transfer).unwrap_or_default();
        let mut receipt = Receipt { records: 0, samples: 0, bytes: 0 };
        for batch in batches.batches {
            receipt.records += batch.records.len() as u64;
            receipt.samples += batch.samples.len() as u64;
            receipt.bytes += batch.bytes.len() as u64;
            let bytes = batch.memory_size();
            endpoints.send_with(bytes, || Ok(batch)).await?;
        }
        Ok(receipt.into())
    }
}

#[cfg(feature = "runtime")]
impl harmos::Operation for Put {
    type Request = PutRequest;
    type Response = Receipt;
    type Error = crate::Error;
    type Endpoints = ();

    async fn initialize(context: harmos::OperationContext) -> Result<Self, Self::Error> {
        Ok(Self { store: context.dependency()? })
    }

    async fn run(
        &mut self, _: harmos::OperationContext, request: &mut Self::Request, _: &mut Self::Endpoints,
    ) -> Result<harmos::Response<Self::Response>, Self::Error> {
        let mut stores = self.store.batches.lock().unwrap();
        if request.first {
            stores.remove(&request.transfer);
        }
        let retained: usize = stores.values().map(|stored| stored.bytes).sum();
        let bytes = retained.saturating_add(request.batch.memory_size());
        if bytes > 64 * 1024 * 1024 {
            return Err(crate::Error::Stream(harmos::operations::StreamError::Capacity));
        }
        let empty = Batch { records: Vec::new(), samples: Vec::new(), bytes: Vec::new() };
        let batch = std::mem::replace(&mut request.batch, empty);
        let receipt = Receipt { records: batch.records.len() as u64, samples: batch.samples.len() as u64, bytes: batch.bytes.len() as u64 };
        let stored = stores.entry(request.transfer).or_default();
        stored.bytes += batch.memory_size();
        stored.batches.push_back(batch);
        Ok(receipt.into())
    }
}

#[cfg(feature = "runtime")]
impl harmos::Operation for Take {
    type Request = TakeRequest;
    type Response = Batch;
    type Error = crate::Error;
    type Endpoints = ();

    async fn initialize(context: harmos::OperationContext) -> Result<Self, Self::Error> {
        Ok(Self { store: context.dependency()? })
    }

    async fn run(
        &mut self, _: harmos::OperationContext, request: &mut Self::Request, _: &mut Self::Endpoints,
    ) -> Result<harmos::Response<Self::Response>, Self::Error> {
        let mut stores = self.store.batches.lock().unwrap();
        let Some(stored) = stores.get_mut(&request.transfer) else {
            return Ok(Batch { records: Vec::new(), samples: Vec::new(), bytes: Vec::new() }.into());
        };
        let batch = stored.batches.pop_front().unwrap_or_else(|| Batch { records: Vec::new(), samples: Vec::new(), bytes: Vec::new() });
        stored.bytes = stored.bytes.saturating_sub(batch.memory_size());
        if stored.batches.is_empty() {
            stores.remove(&request.transfer);
        }
        Ok(batch.into())
    }
}

#[cfg(feature = "runtime")]
impl harmos::Operation for Hash {
    type Request = HashRequest;
    type Response = harmos::domain::model::storage::FileHash;
    type Error = crate::Error;
    type Endpoints = ();

    async fn initialize(context: harmos::OperationContext) -> Result<Self, Self::Error> {
        Ok(Self { provider: context.dependency()? })
    }

    async fn run(
        &mut self, context: harmos::OperationContext, request: &mut Self::Request, _: &mut (),
    ) -> Result<harmos::Response<Self::Response>, Self::Error> {
        Ok((self.provider.hasher)(context, request.request.clone()).await?.into())
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
