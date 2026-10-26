/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Benchmark-Only Streaming Store Shared By Native And Wasm Fixtures.

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::collections::{BTreeMap, VecDeque};
use std::sync::{Arc, Mutex};

// Third Party Imports

// Local Imports
use crate::carrier;
use carrier::Host;
use carrier::MemorySize;
use carrier::operations::{StreamInput, StreamOutput};
use schematic_domain::operations::transfer::{Batch, Download, Hash, Put, Receipt, Take, Upload};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

type HashError = <crate::TransferContext as carrier::Host>::Error;
type HashResponse = <Hash as carrier::operations::OperationDeclaration>::Response;

/* === Structs === */

#[derive(Default)]
pub struct Store {
    batches: Mutex<BTreeMap<u64, Stored>>,
}

#[derive(Default)]
struct Stored {
    bytes: usize,
    batches: VecDeque<Batch>,
}

pub struct Putting {
    store: Arc<Store>,
}

pub struct Taking {
    store: Arc<Store>,
}

pub struct Uploading {
    store: Arc<Store>,
}

pub struct Downloading {
    store: Arc<Store>,
}

pub struct Hashing;

/* === Enums === */

#[carrier::catalog]
pub enum Operations {
    Upload(Uploading),
    Download(Downloading),
    Put(Putting),
    Take(Taking),
    Hash(Hashing),
}

/* === Traits === */

/* === Implementations === */

#[carrier::operation]
impl Uploading {
    async fn initialize(store: Arc<Store>, _: &crate::TransferContext) -> Self {
        Self { store }
    }

    async fn run(
        &mut self, _: &crate::TransferContext, request: &mut Upload, input: &mut StreamInput<Batch>,
    ) -> Result<Receipt, carrier::operations::streaming::Error> {
        let mut batches = VecDeque::new();
        let mut retained = 0usize;
        let mut receipt = Receipt { records: 0, samples: 0, bytes: 0 };
        while let Some(item) = input.recv().await {
            let (batch, _claims) = item.into_owned_request()?;
            receipt.records += batch.records.len() as u64;
            receipt.samples += batch.samples.len() as u64;
            receipt.bytes += batch.bytes.len() as u64;
            retained = retained.saturating_add(batch.memory_size());
            if retained > 64 * 1024 * 1024 {
                return Err(carrier::operations::streaming::Error::Capacity);
            }
            batches.push_back(batch);
        }
        let mut stores = self.store.batches.lock().unwrap();
        let other_bytes: usize = stores.iter().filter(|(id, _)| **id != request.transfer).map(|(_, stored)| stored.bytes).sum();
        if retained.saturating_add(other_bytes) > 64 * 1024 * 1024 {
            return Err(carrier::operations::streaming::Error::Capacity);
        }
        stores.insert(request.transfer, Stored { bytes: retained, batches });
        Ok(receipt)
    }
}

#[carrier::operation]
impl Downloading {
    async fn initialize(store: Arc<Store>, _: &crate::TransferContext) -> Self {
        Self { store }
    }

    async fn run(
        &mut self, _: &crate::TransferContext, request: &mut Download, output: &mut StreamOutput<Batch>,
    ) -> Result<Receipt, carrier::operations::streaming::Error> {
        let batches = self.store.batches.lock().unwrap().remove(&request.transfer).unwrap_or_default();
        let mut receipt = Receipt { records: 0, samples: 0, bytes: 0 };
        for batch in batches.batches {
            receipt.records += batch.records.len() as u64;
            receipt.samples += batch.samples.len() as u64;
            receipt.bytes += batch.bytes.len() as u64;
            let bytes = batch.memory_size();
            output.send_with(bytes, || Ok(batch)).await?;
        }
        Ok(receipt)
    }
}

#[carrier::operation]
impl Putting {
    async fn initialize(store: Arc<Store>, _: &crate::TransferContext) -> Self {
        Self { store }
    }

    async fn run(&mut self, _: &crate::TransferContext, request: &mut Put) -> Result<Receipt, carrier::operations::streaming::Error> {
        let mut stores = self.store.batches.lock().unwrap();
        if request.first {
            stores.remove(&request.transfer);
        }
        let retained: usize = stores.values().map(|stored| stored.bytes).sum();
        let bytes = retained.saturating_add(request.batch.memory_size());
        if bytes > 64 * 1024 * 1024 {
            return Err(carrier::operations::streaming::Error::Capacity);
        }
        let empty = Batch { records: Vec::new(), samples: Vec::new(), bytes: Vec::new() };
        let batch = std::mem::replace(&mut request.batch, empty);
        let receipt = Receipt { records: batch.records.len() as u64, samples: batch.samples.len() as u64, bytes: batch.bytes.len() as u64 };
        let stored = stores.entry(request.transfer).or_default();
        stored.bytes += batch.memory_size();
        stored.batches.push_back(batch);
        Ok(receipt)
    }
}

#[carrier::operation]
impl Taking {
    async fn initialize(store: Arc<Store>, _: &crate::TransferContext) -> Self {
        Self { store }
    }

    async fn run(&mut self, _: &crate::TransferContext, request: &mut Take) -> Result<Batch, carrier::operations::streaming::Error> {
        let mut stores = self.store.batches.lock().unwrap();
        let Some(stored) = stores.get_mut(&request.transfer) else {
            return Ok(Batch { records: Vec::new(), samples: Vec::new(), bytes: Vec::new() });
        };
        let batch = stored.batches.pop_front().unwrap_or_else(|| Batch { records: Vec::new(), samples: Vec::new(), bytes: Vec::new() });
        stored.bytes = stored.bytes.saturating_sub(batch.memory_size());
        if stored.batches.is_empty() {
            stores.remove(&request.transfer);
        }
        Ok(batch)
    }
}

#[carrier::operation]
impl Hashing {
    async fn initialize(_: Arc<Store>, _: &crate::TransferContext) -> Self {
        Self
    }

    async fn run(&mut self, context: &crate::TransferContext, request: &mut Hash) -> Result<HashResponse, HashError> {
        let response = context.call(request.request.clone()).await?;
        Ok(*response)
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
