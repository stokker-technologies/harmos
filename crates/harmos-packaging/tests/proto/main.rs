/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Protocol Export Ownership Stories

/* === Modules === */

#[allow(dead_code, reason = "shared inert contracts also serve compiler acceptance")]
#[path = "../support/proto_contracts.rs"]
mod contracts;

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports

// Third Party Imports
use prost_reflect::DescriptorPool;

// Local Imports

use contracts::{Portable, Routes};
use harmos_domain::operations::Contracts;
use harmos_packaging::proto::{self, Artifacts};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */

#[test]
fn export_removes_retired() {
    let target = tempfile::tempdir().unwrap();
    let contracts = Contracts::of::<Routes>().unwrap();
    let artifacts = Artifacts::new(&contracts).unwrap();
    let root = proto::generate(&artifacts.operation_contracts().unwrap(), &target.path().join("grpc/application")).unwrap();
    assert_eq!(root, target.path().join("grpc/application"));
    let paths: Vec<_> = artifacts.files().keys().map(|name| root.join(name)).collect();
    assert!(paths.iter().all(|path| path.is_file()));
    std::fs::write(root.join("consumer.txt"), "retained").unwrap();
    let empty = Artifacts::new(std::iter::empty()).unwrap();
    proto::generate(&empty.operation_contracts().unwrap(), &target.path().join("grpc/application")).unwrap();
    assert!(paths.iter().all(|path| !path.exists()));
    assert!(root.join("consumer.txt").is_file());
    assert!(DescriptorPool::decode(std::fs::read(root.join("descriptors.bin")).unwrap().as_slice()).unwrap().services().next().is_none());
}

#[test]
fn independent_schema_catalogs() {
    let target = tempfile::tempdir().unwrap();
    let first = target.path().join("producer-a");
    let second = target.path().join("producer-b");
    let routes = Artifacts::new(&Contracts::of::<Routes>().unwrap()).unwrap();
    let portable = Artifacts::new(&Contracts::of::<Portable>().unwrap()).unwrap();
    let native = proto::generate(&routes.operation_contracts().unwrap(), &first.join("grpc/native")).unwrap();
    let guest = proto::generate(&portable.operation_contracts().unwrap(), &first.join("grpc/portable")).unwrap();
    let peer = proto::generate(&portable.operation_contracts().unwrap(), &second.join("grpc/portable")).unwrap();
    let snapshot = std::fs::read(guest.join("descriptors.bin")).unwrap();
    assert_eq!(snapshot, std::fs::read(peer.join("descriptors.bin")).unwrap());
    for name in portable.files().keys() {
        assert_eq!(std::fs::read(guest.join(name)).unwrap(), std::fs::read(peer.join(name)).unwrap());
    }
    // Repeated And Reduced Exports Affect Only Their Selected Producer And Catalog.
    let repeated = proto::generate(&portable.operation_contracts().unwrap(), &first.join("grpc/portable")).unwrap();
    assert_eq!(std::fs::read(repeated.join("descriptors.bin")).unwrap(), snapshot);
    proto::generate(std::iter::empty(), &first.join("grpc/native")).unwrap();
    assert_eq!(std::fs::read(guest.join("descriptors.bin")).unwrap(), snapshot);
    assert_eq!(std::fs::read(peer.join("descriptors.bin")).unwrap(), snapshot);
    assert!(DescriptorPool::decode(std::fs::read(native.join("descriptors.bin")).unwrap().as_slice()).unwrap().services().next().is_none());
    let manifest = std::fs::read_to_string(guest.join("manifest.txt")).unwrap();
    assert!(manifest.starts_with("descriptors.bin\n"));
    assert!(portable.files().keys().all(|name| manifest.lines().any(|line| line == name)));
}

#[test]
fn cancel_final_response_contracts() {
    let contracts = Contracts::of::<Routes>().unwrap();
    for contract in &contracts {
        let Some(grpc) = contract.grpc() else {
            continue;
        };
        let service = proto::descriptor(&grpc.package, &grpc.service, &[(&grpc.method, contract)]).unwrap();
        let ordinary = service.methods().find(|method| method.name() == grpc.method.as_ref()).unwrap();
        let managed = service.methods().find(|method| method.name() == format!("Run{}", grpc.method)).unwrap();
        assert!(managed.is_client_streaming() && managed.is_server_streaming());
        assert_eq!(managed.input().get_field_by_name("request").unwrap().number(), 1);
        assert_eq!(managed.input().get_field_by_name("cancel").unwrap().number(), 3);
        assert_eq!(managed.output().get_field_by_name("response").unwrap().number(), 2);
        assert_eq!(ordinary.is_client_streaming(), contract.input().is_some());
        assert_eq!(ordinary.is_server_streaming(), contract.output().is_some());
        assert_eq!(managed.input().get_field_by_name("item").is_some(), contract.input().is_some());
        assert_eq!(managed.output().get_field_by_name("item").is_some(), contract.output().is_some());
    }
}
