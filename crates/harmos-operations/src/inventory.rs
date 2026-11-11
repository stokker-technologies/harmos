/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */

pub mod contracts;
pub mod contributions;
mod declaration;
pub mod listeners;
pub mod observers;
pub mod operations;
pub mod publications;

/* === Re-Exports === */

pub use contributions::Section;
pub use declaration::{Declaration, DeclarationError, Facility};

/* === Imports === */

// Standard Library Imports

// Third Party Imports
use serde::Deserialize;
// Local Imports

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StreamDeclaration {
    pub metadata: harmos_domain::Metadata,

    #[serde(default)]
    pub row_metadata: Vec<harmos_domain::StreamField>,

    pub id: String,
    pub window: usize,
    pub row: harmos_domain::schema::graph::Schema,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

pub use harmos_domain::schema::marker::{digits, number, quoted, quoted_len, write};

/* === Private Functions === */

fn retain_schema(
    schema: harmos_domain::schema::graph::Schema, settings: &mut harmos_domain::config::SchemaSettings,
) -> Result<harmos_domain::schema::graph::Schema, harmos_domain::schema::graph::Error> {
    use harmos_domain::MemorySize;
    use harmos_domain::schema::graph::Error;
    let preparation = schema.preparation(settings)?;
    let bytes = schema.memory_size();
    settings.preparation_bytes =
        settings.preparation_bytes.get().checked_sub(bytes).and_then(std::num::NonZeroUsize::new).ok_or(Error::Allowance {
            budget: "memory bytes",
            used: bytes,
            allowed: settings.preparation_bytes.get(),
        })?;
    // Resolve Named Definitions and Validate the Result Under One Shared Work
    // Allowance Across Every Contract, Source And Publication In The Artifact.
    settings.preparation_work = preparation
        .work
        .checked_mul(2)
        .and_then(|work| settings.preparation_work.get().checked_sub(work))
        .and_then(std::num::NonZeroUsize::new)
        .ok_or(Error::Allowance {
            budget: "work",
            used: preparation.work.saturating_mul(2),
            allowed: settings.preparation_work.get(),
        })?;
    Ok(schema)
}

/* === Tests === */

#[cfg(test)]
mod identity_tests {
    #[test]
    fn guest_owner_parity() {
        let cases: Vec<(String, bool)> =
            serde_json::from_str(include_str!("../../harmos-domain/tests/fixtures/owner-identities.json")).unwrap();
        for (owner, valid) in cases {
            assert_eq!(super::contracts::valid_identity("guest", &owner), valid);
            let publications = super::publications::Manifest {
                facility: "guest".into(),
                owner: owner.clone(),
                streams: Vec::new(),
            };
            assert_eq!(super::publications::select(vec![publications], "guest", &owner).is_ok(), valid);
            let observers = super::observers::Manifest {
                facility: "guest".into(),
                owner: owner.clone(),
                observers: Vec::new(),
            };
            assert_eq!(super::observers::select_manifest(vec![observers], "guest", &owner, &[]).is_ok(), valid);
        }
    }
}
