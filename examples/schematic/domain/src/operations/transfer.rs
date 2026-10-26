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

// Standard Library Imports

// Third Party Imports

// Local Imports

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

#[harmos_domain::message]
#[derive(Clone)]
pub struct Record {
    pub sequence: u64,
    pub payload: Vec<u8>,
}

#[harmos_domain::message]
#[derive(Clone)]
pub struct Sample {
    pub sequence: u64,
    pub timestamp: u64,
    pub value: f64,
}

#[harmos_domain::message]
#[derive(Clone)]
pub struct Batch {
    pub records: Vec<Record>,
    pub samples: Vec<Sample>,
    pub bytes: Vec<u8>,
}

#[harmos_domain::message]
#[derive(Clone)]
pub struct Receipt {
    pub records: u64,
    pub samples: u64,
    pub bytes: u64,
}

#[harmos_domain::message]
#[harmos_domain::operation(
    id = "benchmark.upload", version = 1, response = Receipt, input = Batch,
    policy = crate::operations::POLICY,
    visibility = harmos_domain::operations::Visibility::Shared,
    replay = harmos_domain::operations::ReplaySafety::Idempotent,
)]
pub struct Upload {
    pub transfer: u64,
}

#[harmos_domain::message]
#[harmos_domain::operation(
    id = "benchmark.download", version = 1, response = Receipt, output = Batch,
    policy = crate::operations::POLICY,
    visibility = harmos_domain::operations::Visibility::Shared,
    replay = harmos_domain::operations::ReplaySafety::Unknown,
)]
pub struct Download {
    pub transfer: u64,
}

#[harmos_domain::message]
#[harmos_domain::operation(
    id = "benchmark.put", version = 1, response = Receipt,
    policy = crate::operations::POLICY,
    visibility = harmos_domain::operations::Visibility::Shared,
    replay = harmos_domain::operations::ReplaySafety::Unknown,
)]
pub struct Put {
    pub transfer: u64,
    pub first: bool,
    pub batch: Batch,
}

#[harmos_domain::message]
#[harmos_domain::operation(
    id = "benchmark.take", version = 1, response = Batch,
    policy = crate::operations::POLICY,
    visibility = harmos_domain::operations::Visibility::Shared,
    replay = harmos_domain::operations::ReplaySafety::Unknown,
)]
pub struct Take {
    pub transfer: u64,
}

#[harmos_domain::message]
#[harmos_domain::operation(
    id = "benchmark.hash_file", version = 1, response = harmos_domain::model::storage::FileHash,
    policy = crate::operations::POLICY,
    visibility = harmos_domain::operations::Visibility::Shared,
    replay = harmos_domain::operations::ReplaySafety::Idempotent,
)]
pub struct Hash {
    pub request: harmos_domain::operations::storage::HashFile,
}

/* === Enums === */

#[harmos_domain::catalog]
pub enum Transfers {
    Upload(Upload),
    Download(Download),
    Put(Put),
    Take(Take),
    Hash(Hash),
}

#[harmos_domain::catalog]
pub enum GuestRequires {
    HashFile(harmos_domain::operations::storage::HashFile),
}

#[harmos_domain::catalog]
pub enum NativeRequires {
    #[nested]
    Shared(crate::operations::Requires),
    HashFile(harmos_domain::operations::storage::HashFile),
}

#[harmos_domain::catalog]
pub enum Guest {
    #[nested]
    Shared(crate::operations::Operations),

    #[nested]
    Transfers(Transfers),
}

#[harmos_domain::catalog]
pub enum Native {
    #[nested]
    Shared(crate::operations::Native),

    #[nested]
    Transfers(Transfers),
}

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
