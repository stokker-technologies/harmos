/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Generate Rust Messages from the Shared Prototype Schema

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::fmt::Write;
use std::path::PathBuf;

// Third Party Imports
use sha2::{Digest, Sha256};

// Local Imports

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

struct Routes;

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl prost_build::ServiceGenerator for Routes {
    fn generate(&mut self, service: prost_build::Service, output: &mut String) {
        let name = format!("{}.{}", service.package, service.proto_name);
        let prefix = service.name.to_uppercase();
        writeln!(output, "pub const {prefix}_NAME: &str = {name:?};").expect("write generated source");
        for method in service.methods {
            let constant = format!("{prefix}_{}", method.name.to_uppercase());
            let path = format!("/{name}/{}", method.proto_name);
            writeln!(output, "pub const {constant}: &str = {path:?};").expect("write generated source");
        }
    }
}

/* === Public Functions === */

/* === Private Functions === */

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Generate From The Only Wire Definition.
    let schema = "proto/sidecar.proto";
    println!("cargo:rerun-if-changed={schema}");
    let mut config = prost_build::Config::new();
    config.bytes([".harmos.prototype.Packet.payload"]);
    config.service_generator(Box::new(Routes));
    config.compile_protos(&[schema], &["proto"])?;

    // Normalize Checkout Line Endings For The Review Contract Identity.
    let source = std::fs::read_to_string(schema)?.replace("\r\n", "\n");
    let fingerprint = Sha256::digest(source.as_bytes());
    let output = PathBuf::from(std::env::var_os("OUT_DIR").ok_or("OUT_DIR missing")?);
    std::fs::write(output.join("schema.sha256"), fingerprint)?;
    Ok(())
}

/* === Tests === */
