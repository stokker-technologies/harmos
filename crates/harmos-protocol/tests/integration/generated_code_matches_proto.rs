/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Check the Committed Bindings Against the Wire Definition.

/* === Modules === */

#[path = "../../codegen.rs"]
mod codegen;

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::{fs, path::Path};

// Third Party Imports

// Local Imports

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
fn generated_code_matches_proto() -> Result<(), Box<dyn std::error::Error>> {
    // Locate the Wire Definition and Disposable Output.
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let output_root = root.join("target/generated/rust/harmos-protocol");
    fs::create_dir_all(&output_root)?;
    let output = tempfile::tempdir_in(output_root)?;

    // Archived Tests Consume the Transferred Compiler, Not the Producer's Registry.
    let (compiler, includes) = if let Ok(profile) = std::env::var("STKR_TEST_PROFILE") {
        let target = format!("{}-{}", std::env::consts::OS, std::env::consts::ARCH);
        let vendor = root.join("target/build").join(profile).join(target).join("fixtures/inputs/vendor");
        let platform = match (std::env::consts::OS, std::env::consts::ARCH) {
            ("linux", "x86_64") => "linux-x86_64",
            ("linux", "aarch64") => "linux-aarch_64",
            ("macos", "x86_64") => "macos-x86_64",
            ("macos", "aarch64") => "macos-aarch_64",
            ("windows", _) => "win32",
            other => panic!("no vendored protoc for {other:?}"),
        };
        let prefix = format!("protoc-bin-vendored-{platform}-");
        let package = fs::read_dir(vendor)?
            .map(|entry| entry.map(|entry| entry.path()))
            .collect::<Result<Vec<_>, _>>()?
            .into_iter()
            .find(|path| path.file_name().unwrap().to_string_lossy().starts_with(&prefix))
            .expect("the transferred vendor inputs contain the pinned protobuf compiler");
        let executable = if cfg!(windows) { "protoc.exe" } else { "protoc" };
        (package.join("bin").join(executable), package.join("include"))
    } else {
        (protoc_bin_vendored::protoc_bin_path()?, protoc_bin_vendored::include_path()?)
    };
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut permissions = fs::metadata(&compiler)?.permissions();
        permissions.set_mode(permissions.mode() | 0o100);
        fs::set_permissions(&compiler, permissions)?;
    }

    // Regenerate With the Pinned Compiler, Without Mutating the Environment.
    codegen::generate(&compiler, &includes, output.path())?;

    // Compare the Complete Generated Source, Retaining Evidence on Failure.
    let generated = fs::read(output.path().join("harmos.v1.rs"))?;
    let committed = fs::read(root.join("crates/harmos-protocol/src/v1.rs"))?;
    if generated != committed {
        let output = output.keep();
        panic!("generated Rust differs from src/v1.rs; regenerated source retained in {}", output.display());
    }
    let generated = fs::read(output.path().join("harmos.host.v1.rs"))?;
    let committed = fs::read(root.join("crates/harmos-protocol/src/host.rs"))?;
    if generated != committed {
        let output = output.keep();
        panic!("generated Rust differs from src/host.rs; regenerated source retained in {}", output.display());
    }
    assert_eq!(fs::read(output.path().join("harmos.storage.v1.rs"))?, include_bytes!("../../src/storage.rs"));
    assert_eq!(fs::read(output.path().join("reflection.rs"))?, include_bytes!("../../src/reflection.rs"));
    assert_eq!(fs::read(output.path().join("descriptors.bin"))?, include_bytes!("../../src/descriptors.bin"));
    assert_eq!(fs::read(output.path().join("guest-descriptors.bin"))?, include_bytes!("../../src/guest-descriptors.bin"));
    codegen::generate_example(&compiler, &includes, output.path())?;
    for (generated, committed) in
        [("schematic.v1.rs", "messages.rs"), ("schematic.reflection.rs", "reflection.rs"), ("schematic.descriptors.bin", "descriptors.bin")]
    {
        assert_eq!(fs::read(output.path().join(generated))?, fs::read(root.join("examples/schematic/protocol/src").join(committed))?);
    }
    Ok(())
}
