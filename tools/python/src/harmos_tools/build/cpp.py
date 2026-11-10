# ***
#
# Harmos
#
# SPDX-License-Identifier: Apache-2.0
#
# ***


"""Build The Native C++ Sidecar Prototype And Its Inert Metadata."""

# === Imports === #

# Standard Library Imports
import argparse
import json
import os
import shutil
import subprocess
import sys
from pathlib import Path

# Third Party Imports

# Local Imports
from ..process import spawn_flags
from . import cargo

# Type Checking Imports


# === Public Exports === #

# === Constants === #

# === Types === #

# === Classes === #

# === Public Functions === #


def main() -> None:
    parser = argparse.ArgumentParser(description="Build The Harmos C++ Prototype")
    parser.add_argument("--test", action="store_true")
    options = parser.parse_args()
    root = Path.cwd()
    environment = dict(os.environ, CARGO_BUILD_WARNINGS="deny")
    selectors = ["--locked", "-p", "harmos-ffi", "--lib"]

    os.environ.update(environment)

    # Compile The Rust ABI.
    directory = root / "target/cpp"
    directory.mkdir(parents=True, exist_ok=True)
    messages = directory / "compiler.json"
    with messages.open("w", encoding="utf-8") as output:
        subprocess.run(
            ["cargo", "build", *selectors, "--message-format=json-render-diagnostics"],
            cwd=root,
            env=environment,
            stdout=output,
            check=True,
            creationflags=spawn_flags(),
        )
    events = [json.loads(line) for line in messages.read_text(encoding="utf-8").splitlines()]
    built = next(event for event in events if event.get("reason") == "compiler-artifact" and event["target"]["name"] == "harmos_ffi")
    library = next(Path(path) for path in built["filenames"] if path.endswith((".lib", ".a")))
    generated = next(event for event in events if event.get("reason") == "build-script-executed" and "harmos-ffi" in event["package_id"])
    header = root / "crates/harmos-ffi/include/harmos.h"
    header.parent.mkdir(parents=True, exist_ok=True)
    shutil.copyfile(Path(generated["out_dir"]) / "harmos.h", header)

    # Locate The Native Compiler And Configure CMake.
    configure = [
        "cmake",
        "-S",
        str(root / "sdk/cpp"),
        "-B",
        str(directory / "build"),
        f"-DHARMOS_FFI_LIBRARY={library}",
        "-DCMAKE_BUILD_TYPE=RelWithDebInfo",
    ]
    generator = []
    if sys.platform == "win32":
        vswhere = Path(os.environ["ProgramFiles(x86)"]) / "Microsoft Visual Studio/Installer/vswhere.exe"
        instance = subprocess.check_output(
            [
                str(vswhere),
                "-latest",
                "-products",
                "*",
                "-requires",
                "Microsoft.VisualStudio.Component.VC.Tools.x86.x64",
                "-property",
                "installationPath",
            ],
            text=True,
            creationflags=spawn_flags(),
        ).strip()
        generator = ["-G", "Visual Studio 17 2022", "-A", "x64", f"-DCMAKE_GENERATOR_INSTANCE={instance}"]
    configure.extend(generator)
    subprocess.run(configure, cwd=root, env=environment, check=True, creationflags=spawn_flags())
    subprocess.run(
        ["cmake", "--build", str(directory / "build"), "--config", "RelWithDebInfo", "--target", "harmos-bench", "--parallel", "2"],
        cwd=root,
        env=environment,
        check=True,
        creationflags=spawn_flags(),
    )

    # Retain The Linker Output And Stamp A Separate Host-Inspectable Executable.
    executable = "harmos-bench.exe" if sys.platform == "win32" else "harmos-bench"
    source = directory / "build" / "RelWithDebInfo" / executable if sys.platform == "win32" else directory / "build" / executable
    if options.test:
        # Verify The Installed Package With The Standalone Author Example.
        prefix = directory / "install"
        subprocess.run(
            ["cmake", "--install", str(directory / "build"), "--config", "RelWithDebInfo", "--prefix", str(prefix)],
            cwd=root,
            env=environment,
            check=True,
            creationflags=spawn_flags(),
        )
        consumer = directory / "consumer"
        configure = [
            "cmake",
            "-S",
            str(root / "sdk/cpp/examples/bench"),
            "-B",
            str(consumer),
            f"-DCMAKE_PREFIX_PATH={prefix}",
            "-DCMAKE_BUILD_TYPE=RelWithDebInfo",
            *generator,
        ]
        subprocess.run(configure, cwd=root, env=environment, check=True, creationflags=spawn_flags())
        subprocess.run(
            ["cmake", "--build", str(consumer), "--config", "RelWithDebInfo", "--parallel", "2"],
            cwd=root,
            env=environment,
            check=True,
            creationflags=spawn_flags(),
        )
        source = consumer / "RelWithDebInfo" / executable if sys.platform == "win32" else consumer / executable

    markers = subprocess.check_output([str(source), "--harmos-export-artifact-v1"], cwd=root, creationflags=spawn_flags())
    destination = directory / executable
    shutil.copy2(source, destination)
    with destination.open("ab") as output:
        output.write(markers)
    print(f"C++ Sidecar: {destination}", flush=True)

    # Run Only The Explicit Native Story And ABI Panic Proof.
    if options.test:
        environment["HARMOS_CPP_SIDECAR"] = str(destination)
        environment["STKR_TEST_REPORT_DIR"] = str(directory)
        os.environ.update(environment)
        status = cargo.focused(
            root,
            "test",
            [
                "--locked",
                "-p",
                "harmos-runtime",
                "--features",
                "cpp-prototype",
                "--test",
                "cpp",
                "--",
                "--exact",
                "cpp_sidecar_walks",
                "--nocapture",
            ],
        )
        if status:
            raise SystemExit(status)
        status = cargo.focused(root, "test", ["--locked", "-p", "harmos-ffi", "--tests", "--", "panic_is_contained_at_the_abi_boundary"])
        raise SystemExit(status)


# === Private Functions === #

# === Entry Point === #

if __name__ == "__main__":
    main()
