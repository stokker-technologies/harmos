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

// Third Party Imports

// Local Imports
use harmos_runtime::storage::{FileMetadata, FileReference, ObjectVersionToken, StorageCursor, StoreHistory};
use harmos_runtime::{Identifier, Message, message, schema::Described};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */
#[message]
#[derive(PartialEq)]
struct Revision {
    cursor: StorageCursor,
    version: ObjectVersionToken,
    file: FileReference,
}

#[message]
struct Seal {
    cursor: Option<StorageCursor>,
    version: Option<ObjectVersionToken>,
    file: Option<FileReference>,
}

/* === Enums === */
#[message]
#[derive(PartialEq)]
enum Source {
    Stored { cursor: StorageCursor },
    Empty,
}

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

/* === Private Functions === */
fn revision() -> Revision {
    let history = StoreHistory { store: Identifier::new("store").unwrap(), epoch: Identifier::new("epoch").unwrap() };
    let cursor = StorageCursor { history, sequence: 7 };
    // Decode The Published Native Codec; Keep The Token Opaque
    let version = ObjectVersionToken::decode(&[5, b's', b't', b'o', b'r', b'e', 5, b'e', b'p', b'o', b'c', b'h', 7]).unwrap();
    let file = FileReference {
        position: StorageCursor::decode(&cursor.encode_to_vec()).unwrap(),
        metadata: FileMetadata {
            id: Identifier::new("file").unwrap(),
            scope: None,
            media_type: "application/octet-stream".into(),
            expected_length: None,
            expected_sha256: None,
        },
        length: 3,
        sha256: [9; 32],
    };
    Revision { cursor, version, file }
}

/* === Tests === */

#[test]
fn values_roundtrip_prost() {
    let value = revision();
    let bytes = value.encode_to_vec();
    assert_eq!(bytes.len(), value.encoded_len());
    assert_eq!(Revision::decode(&bytes).unwrap(), value);
    assert!(Revision::decode(&[]).is_err());
    let value = Source::Stored { cursor: value.cursor };
    let bytes = value.encode_to_vec();
    assert_eq!(bytes.len(), value.encoded_len());
    assert_eq!(Source::decode(&bytes).unwrap(), value);
    assert!(Source::decode(&[]).is_err());
    let empty = Seal::default();
    assert!(empty.cursor.is_none() && empty.version.is_none() && empty.file.is_none());
    for field in Revision::FIELDS {
        assert!(matches!(field.kind, harmos_runtime::schema::graph::Kind::Message { .. }));
    }
}
