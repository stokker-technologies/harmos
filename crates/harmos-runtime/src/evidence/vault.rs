/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

use std::future::Future;
use std::path::PathBuf;
use std::pin::Pin;
use std::time::Duration;

use super::EvidenceError;

pub(crate) type Answering<'a, T> = Pin<Box<dyn Future<Output = Result<T, EvidenceError>> + Send + 'a>>;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Excerpt {
    pub requested: u64,
    pub offset: u64,
    pub bytes: Vec<u8>,
    pub length: u64,
}

pub trait Vault: Send + Sync + 'static {
    fn ceiling(&self) -> usize;

    fn cadence(&self) -> Duration;

    fn open(&self, name: &str) -> impl Future<Output = Result<u64, EvidenceError>> + Send;

    fn external(&self, name: &str) -> impl Future<Output = Result<Option<PathBuf>, EvidenceError>> + Send;

    fn append(&self, name: &str, bytes: &[u8]) -> impl Future<Output = Result<u64, EvidenceError>> + Send;

    fn length(&self, name: &str) -> impl Future<Output = Result<u64, EvidenceError>> + Send;

    fn verify(&self, name: &str) -> impl Future<Output = Result<(u64, u64), EvidenceError>> + Send;

    fn claim(&self, name: &str) -> impl Future<Output = Result<(), EvidenceError>> + Send;

    fn quarantine(&self, name: &str) -> impl Future<Output = Result<(), EvidenceError>> + Send;

    fn read(&self, name: &str, offset: u64, limit: usize) -> impl Future<Output = Result<(Vec<u8>, u64), EvidenceError>> + Send;

    fn read_draft(&self, name: &str, offset: u64, limit: usize) -> impl Future<Output = Result<(Vec<u8>, u64), EvidenceError>> + Send;
}

pub(crate) trait Keeping: Send + Sync + 'static {
    fn ceiling(&self) -> usize;

    fn cadence(&self) -> Duration;

    fn open<'a>(&'a self, name: &'a str) -> Answering<'a, u64>;

    fn external<'a>(&'a self, name: &'a str) -> Answering<'a, Option<PathBuf>>;

    fn append<'a>(&'a self, name: &'a str, bytes: &'a [u8]) -> Answering<'a, u64>;

    fn length<'a>(&'a self, name: &'a str) -> Answering<'a, u64>;

    fn verify<'a>(&'a self, name: &'a str) -> Answering<'a, (u64, u64)>;

    fn claim<'a>(&'a self, name: &'a str) -> Answering<'a, ()>;

    fn quarantine<'a>(&'a self, name: &'a str) -> Answering<'a, ()>;

    fn read<'a>(&'a self, name: &'a str, offset: u64, limit: usize) -> Answering<'a, (Vec<u8>, u64)>;

    fn read_draft<'a>(&'a self, name: &'a str, offset: u64, limit: usize) -> Answering<'a, (Vec<u8>, u64)>;
}

impl Excerpt {
    pub fn next(&self) -> u64 {
        self.offset + self.bytes.len() as u64
    }
}

impl<V: Vault> Keeping for V {
    fn ceiling(&self) -> usize {
        Vault::ceiling(self)
    }

    fn cadence(&self) -> Duration {
        Vault::cadence(self)
    }

    fn open<'a>(&'a self, name: &'a str) -> Answering<'a, u64> {
        Box::pin(Vault::open(self, name))
    }

    fn external<'a>(&'a self, name: &'a str) -> Answering<'a, Option<PathBuf>> {
        Box::pin(Vault::external(self, name))
    }

    fn append<'a>(&'a self, name: &'a str, bytes: &'a [u8]) -> Answering<'a, u64> {
        Box::pin(Vault::append(self, name, bytes))
    }

    fn length<'a>(&'a self, name: &'a str) -> Answering<'a, u64> {
        Box::pin(Vault::length(self, name))
    }

    fn verify<'a>(&'a self, name: &'a str) -> Answering<'a, (u64, u64)> {
        Box::pin(Vault::verify(self, name))
    }

    fn claim<'a>(&'a self, name: &'a str) -> Answering<'a, ()> {
        Box::pin(Vault::claim(self, name))
    }

    fn quarantine<'a>(&'a self, name: &'a str) -> Answering<'a, ()> {
        Box::pin(Vault::quarantine(self, name))
    }

    fn read<'a>(&'a self, name: &'a str, offset: u64, limit: usize) -> Answering<'a, (Vec<u8>, u64)> {
        Box::pin(Vault::read(self, name, offset, limit))
    }

    fn read_draft<'a>(&'a self, name: &'a str, offset: u64, limit: usize) -> Answering<'a, (Vec<u8>, u64)> {
        Box::pin(Vault::read_draft(self, name, offset, limit))
    }
}
