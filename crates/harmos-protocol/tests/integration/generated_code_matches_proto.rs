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
use std::env::consts::{ARCH, OS};
use std::{env, error::Error, fs, path::Path};

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
fn generated_code_matches_proto() -> Result<(), Box<dyn Error>> {
    // Locate the Wire Definition and Disposable Output.
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let output_root = root.join("target/generated/rust/harmos-protocol");
    fs::create_dir_all(&output_root)?;
    let output = tempfile::tempdir_in(output_root)?;

    // Archived Tests Consume the Transferred Compiler, Not the Producer's Registry.
    let (compiler, includes) = if let Ok(profile) = env::var("STKR_TEST_PROFILE") {
        let target = format!("{}-{}", OS, ARCH);
        let vendor = root.join("target/build").join(profile).join(target).join("fixtures/inputs/vendor");
        let platform = match (OS, ARCH) {
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
    for entry in fs::read_dir(output.path())? {
        let path = entry?.path();
        let name = path.file_name().unwrap().to_string_lossy();
        if !name.starts_with("harmos.") || !name.ends_with(".rs") {
            continue;
        }
        let module = codegen::module_path(name.trim_end_matches(".rs"))?;
        let committed = root.join(format!("crates/harmos-protocol/src/{}.rs", module.replace("::", "/")));
        assert_eq!(fs::read(path)?, fs::read(committed)?, "{module}");
    }
    assert_eq!(fs::read(output.path().join("reflection.rs"))?, include_bytes!("../../src/reflection.rs"));
    assert_eq!(fs::read(output.path().join("descriptors.bin"))?, include_bytes!("../../src/descriptors.bin"));
    assert_eq!(fs::read(output.path().join("guest-descriptors.bin"))?, include_bytes!("../../src/guest-descriptors.bin"));
    assert_eq!(fs::read(output.path().join("clock-descriptors.bin"))?, include_bytes!("../../src/clock-descriptors.bin"));
    assert_eq!(
        fs::read(output.path().join("message-options-descriptors.bin"))?,
        include_bytes!("../../src/message-options-descriptors.bin")
    );
    for entry in fs::read_dir(output.path().join("context-types"))? {
        let path = entry?.path();
        assert_eq!(fs::read(&path)?, fs::read(root.join("sdk/rust/src/context/types").join(path.file_name().unwrap()))?);
    }
    codegen::generate_client_fixture(&compiler, &includes, output.path())?;
    for name in [
        "application.host.v1.rs",
        "application.host.v1.jobs.rs",
        "application.host.v1.plugins.rs",
        "client-descriptors.bin",
        "client-reflection.rs",
    ] {
        assert_eq!(fs::read(output.path().join(name))?, fs::read(root.join("sdk/rust/tests/fixtures/client").join(name))?);
    }
    codegen::generate_conformance(&compiler, &includes, output.path())?;
    for (source, target) in [
        ("conformance.model.v1.rs", "generated.rs"),
        ("conformance-descriptors.bin", "descriptors.bin"),
        ("caller-request.bin", "caller-request.bin"),
    ] {
        assert_eq!(fs::read(output.path().join(source))?, fs::read(root.join("sdk/rust/tests/fixtures/conformance").join(target))?);
    }
    Ok(())
}
