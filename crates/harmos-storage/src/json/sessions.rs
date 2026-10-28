/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Durable File Session Fences, Prefix Checkpoints And Mutation Receipts.

use std::{
    fs::{self, OpenOptions},
    io::{Read, Seek, SeekFrom},
    time::SystemTime,
};

use serde::Deserialize;
use sha2::{Digest, Sha256};

use super::{
    files::{Files, Production},
    format,
};
use crate::{FileCheckpoint, FilePhase, FileProgress, MutationReceipt, MutationValue, ReadLimits, Result};

#[derive(Deserialize)]
struct Progress {
    checkpoint: Option<Length>,
    finish: Option<Length>,
    phase: FilePhase,
    deleted: bool,
}

#[derive(Deserialize)]
struct Length {
    length: u64,
}

impl Files {
    pub fn fence(&self, key: &str, expected: u64, next: u64) -> Result<()> {
        if expected.checked_add(1) != Some(next) {
            return Err(format::invalid("file generation must advance once"));
        }

        let mut production = self.load(key)?;
        if production.generation == 0 || production.phase != FilePhase::Producing {
            return Err(format::conflict("file session is not producing"));
        }
        if production.generation != expected && production.generation != next {
            return Err(format::conflict("file generation differs"));
        }
        let checkpoint = production.checkpoint.as_mut().ok_or_else(|| format::corrupt("production has no saved checkpoint"))?;
        production.generation = next;
        checkpoint.session.generation = next;
        let length = checkpoint.length;
        self.save(key, &production)?;
        let path = self.stage_path(key);
        format::regular(&path)?;
        let file = OpenOptions::new().write(true).open(path).map_err(format::io)?;
        if file.metadata().map_err(format::io)?.len() < length {
            return Err(format::corrupt("saved production prefix is missing"));
        }
        file.set_len(length).map_err(format::io)?;
        file.sync_all().map_err(format::io)
    }

    pub fn checkpoint(&self, key: &str, checkpoint: &FileCheckpoint) -> Result<()> {
        self.history.check(&checkpoint.session.history)?;
        let expected = checkpoint.sha256()?;

        let production =
            format::load::<Production>(&self.metadata_path(key), self.maximum)?.ok_or_else(|| format::invalid("unknown staged file"))?;
        if production.phase != FilePhase::Producing
            || production.generation != checkpoint.session.generation
            || production.metadata != checkpoint.metadata
        {
            return Err(format::conflict("checkpoint session or metadata differs"));
        }
        let prior = production.checkpoint.as_ref().ok_or_else(|| format::corrupt("production has no checkpoint"))?;
        if checkpoint.length < prior.length || checkpoint.length == prior.length && checkpoint != prior {
            return Err(format::conflict("checkpoint rewrites a saved prefix"));
        }
        let path = self.stage_path(key);
        format::regular(&path)?;
        let mut file = OpenOptions::new().read(true).write(true).open(path).map_err(format::io)?;
        if file.metadata().map_err(format::io)?.len() < checkpoint.length {
            return Err(format::corrupt("checkpoint exceeds produced bytes"));
        }
        let mut remaining = checkpoint.length - prior.length;
        file.seek(SeekFrom::Start(prior.length)).map_err(format::io)?;
        let mut buffer = vec![0; 1024 * 1024];
        let mut digest = prior.restore_hash()?;
        while remaining > 0 {
            let count = remaining.min(buffer.len() as u64) as usize;
            file.read_exact(&mut buffer[..count]).map_err(format::io)?;
            digest.update(&buffer[..count]);
            remaining -= count as u64;
        }
        if <[u8; 32]>::from(digest.finalize()) != expected {
            return Err(format::conflict("checkpoint hash differs from produced bytes"));
        }
        file.sync_all().map_err(format::io)?;
        let production = Production {
            metadata: production.metadata,
            generation: production.generation,
            checkpoint: Some(checkpoint),
            finish: production.finish,
            phase: production.phase,
            deleted: production.deleted,
        };
        self.save(key, &production)
    }

    pub fn progress(&self, key: &str) -> Result<FileProgress> {
        let path = self.metadata_path(key);
        format::regular(&path)?;
        let length = usize::try_from(fs::metadata(&path).map_err(format::io)?.len())
            .map_err(|_| format::invalid("production metadata length overflow"))?;
        if length > self.maximum {
            return Err(format::corrupt("production metadata exceeds configured size"));
        }

        let production = format::load::<Progress>(&path, length)?.ok_or_else(|| format::invalid("unknown staged file"))?;
        if production.deleted {
            return Err(format::conflict("file has been deleted"));
        }
        let length = production.finish.or(production.checkpoint).ok_or_else(|| format::invalid("file is not a public production"))?.length;
        Ok(FileProgress { length, phase: production.phase })
    }

    pub fn recover(&self, limits: ReadLimits) -> Result<Vec<FileCheckpoint>> {
        let mut output = Vec::new();
        let mut bytes = 0usize;
        for entry in fs::read_dir(self.root.join("productions")).map_err(format::io)? {
            let path = entry.map_err(format::io)?.path();
            if path.extension().is_none_or(|extension| extension != "json") {
                continue;
            }
            let production = format::load::<Production>(&path, self.maximum)?.ok_or_else(|| format::corrupt("production disappeared"))?;
            if production.deleted || production.phase != FilePhase::Producing {
                continue;
            }
            let Some(checkpoint) = production.checkpoint else {
                continue;
            };
            drop(production.metadata);
            drop(production.finish);
            bytes = bytes
                .checked_add(crate::manager::encoded_size(&checkpoint)?)
                .ok_or_else(|| format::invalid("production recovery size overflow"))?;
            limits.check(output.len() + 1, bytes)?;
            push_recovered(&mut output, checkpoint)?;
        }
        Ok(output)
    }

    pub fn receipts(&self, limits: ReadLimits) -> Result<Vec<MutationReceipt>> {
        let mut receipts = Vec::new();
        let mut bytes = 0usize;
        for entry in fs::read_dir(self.root.join("mutations")).map_err(format::io)? {
            let path = entry.map_err(format::io)?.path();
            if path.extension().is_none_or(|extension| extension != "json") {
                continue;
            }
            let receipt = format::load::<MutationReceipt>(&path, self.maximum)?.ok_or_else(|| format::corrupt("mutation disappeared"))?;
            if receipt.expires_at <= SystemTime::now() {
                continue;
            }
            let size = crate::manager::encoded_size(&receipt)?;
            if size > limits.bytes.get().saturating_sub(bytes) || receipts.len() == limits.rows.get() {
                break;
            }
            bytes += size;
            push_recovered(&mut receipts, receipt)?;
        }
        Ok(receipts)
    }

    pub fn save_receipt(&self, receipt: &MutationReceipt, saved: u64) -> Result<()> {
        if receipt.expires_at <= SystemTime::now() {
            return Ok(());
        }

        // An Existing Durable Outcome Survives Later Fencing And Deletion.
        let encoded = serde_json::to_vec(&receipt.mutation).map_err(format::json)?;
        let name = format::hex(&Sha256::digest(&encoded));
        let path = self.root.join("mutations").join(format!("{name}.json"));
        if let Some(prior) = format::load::<MutationReceipt>(&path, self.maximum)? {
            if prior != *receipt {
                return Err(format::conflict("mutation retry differs"));
            }
            return Ok(());
        }

        // New Outcomes Must Follow Their Durable Resource Dependencies.
        match &receipt.value {
            MutationValue::Cursor(cursor) => {
                self.history.check(&cursor.history)?;
                if cursor.sequence > saved {
                    return Err(format::conflict("mutation receipt precedes its durable cursor"));
                }
            }
            MutationValue::Deleted(file) => {
                self.history.check(&file.history)?;
                let path = self.metadata_path(&super::files::key(&file.id));
                let production =
                    format::load::<Production>(&path, self.maximum)?.ok_or_else(|| format::invalid("deleted file metadata is missing"))?;
                if !production.deleted {
                    return Err(format::conflict("deletion receipt precedes deletion marker"));
                }
            }
            MutationValue::Producer(producer) => {
                self.history.check(&producer.session.history)?;
                let production = format::load::<Production>(&self.metadata_path(&super::files::key(&producer.session.id)), self.maximum)?
                    .ok_or_else(|| format::invalid("unknown staged file"))?;
                let length = production
                    .finish
                    .as_ref()
                    .map(|value| value.length)
                    .or_else(|| production.checkpoint.as_ref().map(|value| value.length))
                    .unwrap_or(0);
                if production.generation < producer.session.generation || length < producer.length {
                    return Err(format::conflict("producer receipt precedes durable fence or prefix"));
                }
            }
            MutationValue::Reference(reference) => {
                self.history.check(&reference.position.history)?;
                let production = format::load::<Production>(&self.metadata_path(&super::files::key(&reference.metadata.id)), self.maximum)?
                    .ok_or_else(|| format::invalid("unknown staged file"))?;
                if reference.position.sequence > saved
                    || production.phase != FilePhase::Finished
                    || production.finish.as_ref() != Some(reference.as_ref())
                {
                    return Err(format::conflict("finish receipt precedes durable publication"));
                }
            }
            MutationValue::Progress(_) | MutationValue::Rejected => {}
        }
        format::store(&path, receipt, self.maximum)
    }
}

fn push_recovered<T>(output: &mut Vec<T>, value: T) -> Result<()> {
    if output.len() == output.capacity() {
        let capacity = output
            .len()
            .checked_add(1)
            .map(|required| required.max(output.capacity().saturating_mul(2)))
            .ok_or_else(|| format::invalid("recovery collection size overflow"))?;
        output.try_reserve_exact(capacity - output.len()).map_err(|_| format::invalid("recovery collection allocation failed"))?;
    }
    output.push(value);
    Ok(())
}
