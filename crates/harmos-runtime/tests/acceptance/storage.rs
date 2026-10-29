/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Compile Exported Storage Operation Contracts

use super::fixtures;
use harmos_operations::storage::Storage;
use harmos_runtime::operations::Contracts;

#[test]
fn grpc_descriptors_compile() {
    let contracts = Contracts::of::<Storage>().unwrap().into_iter().filter(|contract| contract.grpc().is_some()).collect::<Vec<_>>();
    let artifacts = harmos_packaging::proto::Artifacts::new(&contracts).unwrap();
    let target = tempfile::tempdir().unwrap();
    let root = harmos_packaging::proto::generate(&artifacts.operation_contracts().unwrap(), &target.path().join("storage")).unwrap();
    let output = fixtures::command(fixtures::protoc())
        .current_dir(&root)
        .arg("--proto_path=.")
        .arg(format!("--descriptor_set_out={}", target.path().join("compiled.bin").display()))
        .args(artifacts.files().keys())
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
}
