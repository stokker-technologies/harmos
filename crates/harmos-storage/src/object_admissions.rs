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
use std::num::NonZeroUsize;

// Third Party Imports
use sha2::{Digest, Sha256};

// Local Imports
use super::admissions::Mutation;
use super::objects::invalid;
use super::{ObjectReceipt, ObjectValue, StorageManager};
use crate::Result;
use crate::storage::MutationId;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */
impl<C> StorageManager<C> {
    pub async fn write_objects_once(&self, mutation: &MutationId, objects: &[ObjectValue]) -> Result<Vec<ObjectReceipt>> {
        let count = NonZeroUsize::new(objects.len()).ok_or_else(|| invalid("object batch must not be empty"))?;
        let mut hash = Sha256::new();
        part(&mut hash, b"storage.write_objects:v1");
        hash.update((objects.len() as u64).to_le_bytes());
        for object in objects {
            part(&mut hash, object.schema.id.as_str().as_bytes());
            hash.update(object.schema.version.get().to_le_bytes());
            part(&mut hash, object.item.as_str().as_bytes());
            part(&mut hash, &object.payload);
        }
        self.admit_object_group(
            self.object_admission_bytes(),
            count,
            Some(Mutation { id: mutation, fingerprint: hash.finalize().into() }),
            |group| {
                for object in objects {
                    group.write_value(object)?;
                }
                Ok(())
            },
        )
        .await
    }
}

/* === Public Functions === */

/* === Private Functions === */
fn part(hash: &mut Sha256, bytes: &[u8]) {
    hash.update((bytes.len() as u64).to_le_bytes());
    hash.update(bytes);
}

/* === Tests === */
