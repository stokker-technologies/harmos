/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */
#[cfg(feature = "grpc")]
#[path = "src/contract.rs"]
mod contract;

/* === Re-Exports === */

/* === Imports === */
// Standard Library Imports

// Third Party Imports

// Local Imports
#[cfg(feature = "grpc")]
use harmos::operations::Contracts;

#[cfg(feature = "grpc")]
use contract::{Operations, Payload};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

/* === Private Functions === */
#[cfg(feature = "grpc")]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output = std::path::PathBuf::from(std::env::var_os("OUT_DIR").ok_or("missing OUT_DIR")?);
    std::fs::write(
        output.join("payload.bin"),
        harmos::Protobuf::encode_to_vec(&Payload {
            value: "payload".into(),
        }),
    )?;
    let opening = harmos::grpc::InputEnvelope::<Payload, Payload>::Request(Payload {
        value: "payload".into(),
    });
    let final_response = harmos::grpc::OutputEnvelope::<Payload, Payload>::Response(Payload {
        value: "payload".into(),
    });
    std::fs::write(
        output.join("opening.bin"),
        harmos::Protobuf::encode_to_vec(&opening),
    )?;
    std::fs::write(
        output.join("completion.bin"),
        harmos::Protobuf::encode_to_vec(&final_response),
    )?;
    let contracts = Contracts::of::<Operations>()?;
    harmos::grpc::Artifacts::new(&contracts)?.export_build("operations")?;
    let methods: Vec<_> = ["Unary", "Upload", "Download", "Exchange"]
        .into_iter()
        .zip(&contracts)
        .collect();
    let service = harmos::grpc::descriptor("fixture.v1", "Operations", &methods)?;
    let generated = harmos::grpc::generate_server(
        "fixture.v1",
        "Operations",
        "::harmos",
        &[
            harmos::grpc::Method {
                name: "Unary",
                contract: &contracts[0],
                declaration: "crate::contract::Unary",
            },
            harmos::grpc::Method {
                name: "Upload",
                contract: &contracts[1],
                declaration: "crate::contract::Upload",
            },
            harmos::grpc::Method {
                name: "Download",
                contract: &contracts[2],
                declaration: "crate::contract::Download",
            },
            harmos::grpc::Method {
                name: "Exchange",
                contract: &contracts[3],
                declaration: "crate::contract::Exchange",
            },
        ],
    )?;
    std::fs::write(output.join("server.rs"), generated.source)?;
    let storage_contracts = Contracts::of::<harmos::storage::operations::Operations>()?;
    let storage = harmos::grpc::generate_server(
        "storage.v1",
        "Storage",
        "::harmos",
        &[
            harmos::grpc::Method {
                name: "Save",
                contract: &storage_contracts[0],
                declaration: "::harmos::storage::operations::Save",
            },
            harmos::grpc::Method {
                name: "QueryFiles",
                contract: &storage_contracts[1],
                declaration: "::harmos::storage::operations::QueryFiles",
            },
            harmos::grpc::Method {
                name: "ReadRecords",
                contract: &storage_contracts[2],
                declaration: "::harmos::storage::operations::ReadRecords",
            },
            harmos::grpc::Method {
                name: "QueryStream",
                contract: &storage_contracts[3],
                declaration: "::harmos::storage::operations::QueryStream",
            },
            harmos::grpc::Method {
                name: "Follow",
                contract: &storage_contracts[4],
                declaration: "::harmos::storage::operations::Follow",
            },
            harmos::grpc::Method {
                name: "FollowStream",
                contract: &storage_contracts[5],
                declaration: "::harmos::storage::operations::FollowStream",
            },
            harmos::grpc::Method {
                name: "ReadFiles",
                contract: &storage_contracts[6],
                declaration: "::harmos::storage::operations::ReadFiles",
            },
            harmos::grpc::Method {
                name: "FollowFile",
                contract: &storage_contracts[7],
                declaration: "::harmos::storage::operations::FollowFile",
            },
            harmos::grpc::Method {
                name: "QueryStorage",
                contract: &storage_contracts[8],
                declaration: "::harmos::storage::operations::QueryStorage",
            },
            harmos::grpc::Method {
                name: "ResumeFile",
                contract: &storage_contracts[9],
                declaration: "::harmos::storage::operations::ResumeFile",
            },
            harmos::grpc::Method {
                name: "CreateFile",
                contract: &storage_contracts[10],
                declaration: "::harmos::storage::operations::CreateFile",
            },
            harmos::grpc::Method {
                name: "AppendFile",
                contract: &storage_contracts[11],
                declaration: "::harmos::storage::operations::AppendFile",
            },
            harmos::grpc::Method {
                name: "FinishFile",
                contract: &storage_contracts[12],
                declaration: "::harmos::storage::operations::FinishFile",
            },
            harmos::grpc::Method {
                name: "AbandonFile",
                contract: &storage_contracts[13],
                declaration: "::harmos::storage::operations::AbandonFile",
            },
            harmos::grpc::Method {
                name: "WriteRecords",
                contract: &storage_contracts[14],
                declaration: "::harmos::storage::operations::WriteRecords",
            },
            harmos::grpc::Method {
                name: "WriteStream",
                contract: &storage_contracts[15],
                declaration: "::harmos::storage::operations::WriteStream",
            },
            harmos::grpc::Method {
                name: "DeleteFile",
                contract: &storage_contracts[16],
                declaration: "::harmos::storage::operations::DeleteFile",
            },
            harmos::grpc::Method {
                name: "WriteSchema",
                contract: &storage_contracts[17],
                declaration: "::harmos::storage::operations::WriteSchema",
            },
            harmos::grpc::Method {
                name: "QuerySchemas",
                contract: &storage_contracts[18],
                declaration: "::harmos::storage::operations::QuerySchemas",
            },
            harmos::grpc::Method {
                name: "WriteObjects",
                contract: &storage_contracts[19],
                declaration: "::harmos::storage::operations::WriteObjects",
            },
            harmos::grpc::Method {
                name: "ReadObjects",
                contract: &storage_contracts[20],
                declaration: "::harmos::storage::operations::ReadObjects",
            },
            harmos::grpc::Method {
                name: "FollowObjects",
                contract: &storage_contracts[21],
                declaration: "::harmos::storage::operations::FollowObjects",
            },
            harmos::grpc::Method {
                name: "Acknowledge",
                contract: &storage_contracts[22],
                declaration: "::harmos::storage::operations::Acknowledge",
            },
            harmos::grpc::Method {
                name: "Reductions",
                contract: &storage_contracts[23],
                declaration: "::harmos::storage::operations::Reductions",
            },
        ],
    )?;
    std::fs::write(output.join("storage_server.rs"), storage.source)?;
    let mut fds = prost_reflect::prost_types::FileDescriptorSet {
        file: service
            .parent_pool()
            .file_descriptor_protos()
            .cloned()
            .collect(),
    };
    fds.file.extend(
        storage
            .descriptor
            .parent_pool()
            .file_descriptor_protos()
            .cloned(),
    );
    tonic_prost_build::configure()
        .include_file("generated.rs")
        .compile_fds(fds)?;
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=src/contract.rs");
    Ok(())
}

/* === Tests === */

#[cfg(not(feature = "grpc"))]
fn main() {}
