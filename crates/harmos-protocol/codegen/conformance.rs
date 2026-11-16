/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

//! Generate Conformance Payloads and Caller Probe Bytes.

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::{error::Error, fs, path::Path};

// Third Party Imports
use prost::Message;
use prost_build::Config;
use prost_types::FileDescriptorSet;

// Local Imports
use harmos_protocol::host::InvocationsCreateRequest;
use harmos_protocol::schema;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

pub fn generate_conformance(compiler: &Path, includes: &Path, output: &Path) -> Result<(), Box<dyn Error>> {
    let proto = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../sdk/tests/fixtures/conformance/proto");
    let mut config = Config::new();
    config.protoc_executable(compiler);
    config.out_dir(output);
    config.skip_source_info();
    config.file_descriptor_set_path(output.join("conformance-descriptors.bin"));
    let library = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../proto");
    config.compile_protos(
        &[proto.join("conformance/model/v1/fixture.proto"), proto.join("conformance/host/v1/caller_probe.proto")],
        &[proto, library, includes.into()],
    )?;
    let descriptors: FileDescriptorSet = FileDescriptorSet::decode(fs::read(output.join("conformance-descriptors.bin"))?.as_slice())?;
    let request = InvocationsCreateRequest {
        operation: "conformance.host.v1.CallerProbe/Get".into(),
        fingerprint: schema::fingerprint(
            "conformance.host.v1.CallerProbe/Get",
            &descriptors,
            "conformance.host.v1.CallerProbeGetRequest",
            "conformance.host.v1.CallerProbeGetResponse",
        )?,
        payload: Vec::new(),
    };
    fs::write(output.join("caller-request.bin"), request.encode_to_vec())?;
    let descriptors = schema::operation_types(
        &descriptors,
        &["conformance.model.v1.Graph", "conformance.model.v1.PayloadOnly", "conformance.model.v1.Values"],
    )?;
    fs::write(output.join("conformance-descriptors.bin"), descriptors.encode_to_vec())?;
    Ok(())
}

/* === Private Functions === */

/* === Tests === */
