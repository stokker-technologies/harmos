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
                name: "Query",
                contract: storage_contracts.iter().find(|contract| contract.operation().id.as_str()
                    == <harmos::storage::operations::Query as harmos::operations::OperationDeclaration>::ID)
                    .ok_or("missing Query storage contract")?,
                declaration: "::harmos::storage::operations::Query",
            },
            harmos::grpc::Method {
                name: "ReadFile",
                contract: storage_contracts.iter().find(|contract| contract.operation().id.as_str()
                    == <harmos::storage::operations::ReadFile as harmos::operations::OperationDeclaration>::ID)
                    .ok_or("missing ReadFile storage contract")?,
                declaration: "::harmos::storage::operations::ReadFile",
            },
            harmos::grpc::Method {
                name: "Follow",
                contract: storage_contracts.iter().find(|contract| contract.operation().id.as_str()
                    == <harmos::storage::operations::Follow as harmos::operations::OperationDeclaration>::ID)
                    .ok_or("missing Follow storage contract")?,
                declaration: "::harmos::storage::operations::Follow",
            },
            harmos::grpc::Method {
                name: "FollowStream",
                contract: storage_contracts.iter().find(|contract| contract.operation().id.as_str()
                    == <harmos::storage::operations::FollowStream as harmos::operations::OperationDeclaration>::ID)
                    .ok_or("missing FollowStream storage contract")?,
                declaration: "::harmos::storage::operations::FollowStream",
            },
            harmos::grpc::Method {
                name: "FollowFile",
                contract: storage_contracts.iter().find(|contract| contract.operation().id.as_str()
                    == <harmos::storage::operations::FollowFile as harmos::operations::OperationDeclaration>::ID)
                    .ok_or("missing FollowFile storage contract")?,
                declaration: "::harmos::storage::operations::FollowFile",
            },
            harmos::grpc::Method {
                name: "QueryStorage",
                contract: storage_contracts.iter().find(|contract| contract.operation().id.as_str()
                    == <harmos::storage::operations::QueryStorage as harmos::operations::OperationDeclaration>::ID)
                    .ok_or("missing QueryStorage storage contract")?,
                declaration: "::harmos::storage::operations::QueryStorage",
            },
            harmos::grpc::Method {
                name: "ResumeFile",
                contract: storage_contracts.iter().find(|contract| contract.operation().id.as_str()
                    == <harmos::storage::operations::ResumeFile as harmos::operations::OperationDeclaration>::ID)
                    .ok_or("missing ResumeFile storage contract")?,
                declaration: "::harmos::storage::operations::ResumeFile",
            },
            harmos::grpc::Method {
                name: "CreateFile",
                contract: storage_contracts.iter().find(|contract| contract.operation().id.as_str()
                    == <harmos::storage::operations::CreateFile as harmos::operations::OperationDeclaration>::ID)
                    .ok_or("missing CreateFile storage contract")?,
                declaration: "::harmos::storage::operations::CreateFile",
            },
            harmos::grpc::Method {
                name: "AppendFile",
                contract: storage_contracts.iter().find(|contract| contract.operation().id.as_str()
                    == <harmos::storage::operations::AppendFile as harmos::operations::OperationDeclaration>::ID)
                    .ok_or("missing AppendFile storage contract")?,
                declaration: "::harmos::storage::operations::AppendFile",
            },
            harmos::grpc::Method {
                name: "FinishFile",
                contract: storage_contracts.iter().find(|contract| contract.operation().id.as_str()
                    == <harmos::storage::operations::FinishFile as harmos::operations::OperationDeclaration>::ID)
                    .ok_or("missing FinishFile storage contract")?,
                declaration: "::harmos::storage::operations::FinishFile",
            },
            harmos::grpc::Method {
                name: "AbandonFile",
                contract: storage_contracts.iter().find(|contract| contract.operation().id.as_str()
                    == <harmos::storage::operations::AbandonFile as harmos::operations::OperationDeclaration>::ID)
                    .ok_or("missing AbandonFile storage contract")?,
                declaration: "::harmos::storage::operations::AbandonFile",
            },
            harmos::grpc::Method {
                name: "WriteRecords",
                contract: storage_contracts.iter().find(|contract| contract.operation().id.as_str()
                    == <harmos::storage::operations::WriteRecords as harmos::operations::OperationDeclaration>::ID)
                    .ok_or("missing WriteRecords storage contract")?,
                declaration: "::harmos::storage::operations::WriteRecords",
            },
            harmos::grpc::Method {
                name: "WriteStreams",
                contract: storage_contracts.iter().find(|contract| contract.operation().id.as_str()
                    == <harmos::storage::operations::WriteStreams as harmos::operations::OperationDeclaration>::ID)
                    .ok_or("missing WriteStreams storage contract")?,
                declaration: "::harmos::storage::operations::WriteStreams",
            },
            harmos::grpc::Method {
                name: "WriteFile",
                contract: storage_contracts.iter().find(|contract| contract.operation().id.as_str()
                    == <harmos::storage::operations::WriteFile as harmos::operations::OperationDeclaration>::ID)
                    .ok_or("missing WriteFile storage contract")?,
                declaration: "::harmos::storage::operations::WriteFile",
            },
            harmos::grpc::Method {
                name: "DeleteFile",
                contract: storage_contracts.iter().find(|contract| contract.operation().id.as_str()
                    == <harmos::storage::operations::DeleteFile as harmos::operations::OperationDeclaration>::ID)
                    .ok_or("missing DeleteFile storage contract")?,
                declaration: "::harmos::storage::operations::DeleteFile",
            },
            harmos::grpc::Method {
                name: "WriteSchema",
                contract: storage_contracts.iter().find(|contract| contract.operation().id.as_str()
                    == <harmos::storage::operations::WriteSchema as harmos::operations::OperationDeclaration>::ID)
                    .ok_or("missing WriteSchema storage contract")?,
                declaration: "::harmos::storage::operations::WriteSchema",
            },
            harmos::grpc::Method {
                name: "WriteObjects",
                contract: storage_contracts.iter().find(|contract| contract.operation().id.as_str()
                    == <harmos::storage::operations::WriteObjects as harmos::operations::OperationDeclaration>::ID)
                    .ok_or("missing WriteObjects storage contract")?,
                declaration: "::harmos::storage::operations::WriteObjects",
            },
            harmos::grpc::Method {
                name: "FollowObjects",
                contract: storage_contracts.iter().find(|contract| contract.operation().id.as_str()
                    == <harmos::storage::operations::FollowObjects as harmos::operations::OperationDeclaration>::ID)
                    .ok_or("missing FollowObjects storage contract")?,
                declaration: "::harmos::storage::operations::FollowObjects",
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
