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
use std::path::PathBuf;

// Third Party Imports

// Local Imports
use harmos_domain::model::storage::*;
use harmos_domain::{MemorySize, Message};
use harmos_operations::storage::{CaptureFile, FollowFile, FollowProduction, HashFile, ReadFile};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

/* === Private Functions === */

fn roundtrip<T: Message + MemorySize + harmos_domain::schema::Described + PartialEq + std::fmt::Debug>(value: T) {
    let bytes = value.encode();
    assert_eq!(T::decode(&bytes).unwrap(), value);
    let mut protobuf = Vec::new();
    harmos_sdk::grpc::encode(&value, &mut protobuf).unwrap();
    let prepared = harmos_sdk::grpc::prepare::<T>(harmos_sdk::grpc::DecodeInput::new(&protobuf)).unwrap();
    assert!(prepared.memory() >= value.memory_size());
    assert_eq!(prepared.decode().unwrap(), value);
}

/* === Tests === */

#[test]
fn file_contract_roundtrips() {
    let history = StoreHistory {
        store: harmos_domain::Identifier::new("files").unwrap(),
        epoch: harmos_domain::Identifier::new("first").unwrap(),
    };
    let metadata = FileMetadata {
        id: harmos_domain::Identifier::new("source").unwrap(),
        scope: None,
        media_type: "application/octet-stream".into(),
        expected_length: None,
        expected_sha256: None,
    };
    let file = FileReference {
        position: StorageCursor { history: history.clone(), sequence: 1 },
        metadata: metadata.clone(),
        length: 3,
        sha256: [7; 32],
    };
    let range = ByteRange { offset: 1, limit: Some(2) };
    for source in [FileSource::Stored(file), FileSource::Local(PathBuf::from("measurements/échantillon.bin"))] {
        roundtrip(source.clone());
        roundtrip(ReadFile { source: source.clone(), range });
        roundtrip(HashFile { source: source.clone(), range });
        for consistency in [CaptureConsistency::Bounded, CaptureConsistency::Stable] {
            roundtrip(CaptureFile {
                source: source.clone(),
                range,
                metadata: metadata.clone(),
                consistency,
            });
        }
        for start in [FollowStart::Beginning, FollowStart::End, FollowStart::At(2)] {
            roundtrip(FollowFile { source: source.clone(), start });
        }
    }
    roundtrip(FollowProduction {
        file: StagedFile { history, id: metadata.id },
        offset: 2,
    });
    roundtrip(FileHash { bytes_read: 12345, sha256: [0xa5; 32] });
    roundtrip(FileEvent::Data(FileBatch { offset: 1000, bytes: vec![1, 2, 3] }));
    roundtrip(FileEvent::Truncated { length: 2 });
    roundtrip(FileEvent::Replaced);
    let mut short = FileHash { bytes_read: 1, sha256: [0; 32] }.encode();
    short[1] = 31;
    short.pop();
    assert!(FileHash::decode(&short).is_err());
}

#[cfg(unix)]
#[test]
fn path_transport_rejects() {
    use std::os::unix::ffi::OsStringExt;
    let path = PathBuf::from(std::ffi::OsString::from_vec(vec![b'a', 0xff]));
    let source = FileSource::Local(path);
    assert!(FileSource::decode(&source.encode()).is_err());
    let mut protobuf = Vec::new();
    harmos_sdk::grpc::encode(&source, &mut protobuf).unwrap();
    let prepared = harmos_sdk::grpc::prepare::<FileSource>(harmos_sdk::grpc::DecodeInput::new(&protobuf)).unwrap();
    assert!(prepared.decode().is_err());
}
