/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Canonical Plugin Stream Revisions

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports

// Third Party Imports
use sha2::{Digest as _, Sha256};

// Local Imports

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

// Derives an opaque, nonzero storage revision from a row schema and window.
// The encoding is the domain separator, a little-endian u64 window, and
// `schema::identity::canonical`; the first four SHA-256 bytes are little-endian.
pub fn revision(schema: &crate::schema::graph::Schema, window: usize) -> Result<u32, crate::schema::graph::Error> {
    let canonical = crate::schema::identity::canonical(schema.root()?)?;
    let mut hash = Sha256::new();
    hash.update(b"harmos.stream.v1\0");
    hash.update(u64::try_from(window).map_err(|_| crate::schema::graph::Error::Budget)?.to_le_bytes());
    hash.update(canonical);
    let digest = hash.finalize();
    Ok(u32::from_le_bytes(digest[..4].try_into().expect("four digest bytes")).max(1))
}

/* === Private Functions === */

/* === Tests === */

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema::{
        Cardinality,
        graph::{Field, Kind, Schema},
    };

    #[test]
    fn canonical_stream_revision_golden() {
        let schema = Schema {
            root: 0,
            definitions: vec![vec![Field {
                name: "value".into(),
                tag: 1,
                kind: Kind::U64,
                cardinality: Cardinality::Required,
            }]],
        };
        // Independently Encoded: One Definition, One Field, UTF-8 Name, Tag, Kind, Cardinality.
        let canonical = b"\x01\0\0\0\x01\0\0\0\x05\0\0\0value\x01\0\0\0\x02\0";
        assert_eq!(crate::schema::identity::canonical(schema.root().unwrap()).unwrap(), canonical);
        assert_eq!(revision(&schema, 4).unwrap(), 2_815_093_881);
        assert_ne!(revision(&schema, 5).unwrap(), revision(&schema, 4).unwrap());
        let mut changed = schema.clone();
        changed.definitions[0][0].kind = Kind::String;
        assert_ne!(revision(&changed, 4).unwrap(), revision(&schema, 4).unwrap());
    }

    #[test]
    fn revision_ignores_graph_indices_but_preserves_row_order() {
        let child = vec![Field {
            name: "value".into(),
            tag: 1,
            kind: Kind::U64,
            cardinality: Cardinality::Required,
        }];
        let schema = Schema {
            root: 0,
            definitions: vec![
                vec![
                    Field {
                        name: "first".into(),
                        tag: 1,
                        kind: Kind::Message { definition: 1 },
                        cardinality: Cardinality::Required,
                    },
                    Field {
                        name: "second".into(),
                        tag: 2,
                        kind: Kind::Bool,
                        cardinality: Cardinality::Required,
                    },
                ],
                child,
            ],
        };
        let mut reordered = schema.clone();
        reordered.definitions.swap(0, 1);
        reordered.root = 1;
        reordered.definitions[1][0].kind = Kind::Message { definition: 0 };
        assert_eq!(revision(&schema, 4).unwrap(), revision(&reordered, 4).unwrap());
        reordered.definitions[1].swap(0, 1);
        assert_ne!(revision(&schema, 4).unwrap(), revision(&reordered, 4).unwrap());
    }
}
