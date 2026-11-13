# ***
#
# Harmos
#
# SPDX-License-Identifier: Apache-2.0
#
# ***

"""Build the C++ Plugin SDK and Its Pinned gRPC Dependencies."""

# === Imports === #

# Standard Library Imports
import argparse
import os
import shutil
import subprocess
import sys
import tarfile
import time
from pathlib import Path

# Third Party Imports

# Local Imports
from ..process import spawn_flags

# Type Checking Imports

# === Public Exports === #

# === Constants === #

# === Types === #

# === Classes === #

# === Public Functions === #


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--profile", choices=("dev", "release"), default="dev")
    parser.add_argument("--check", action="store_true")
    parser.add_argument("--generate", action="store_true")
    options = parser.parse_args()
    root = Path.cwd()
    from .resource_roots import generate

    generate(root)
    directory = root / "target/cpp" / options.profile
    configuration = "Debug" if options.profile == "dev" else "Release"
    configure = [
        "cmake",
        "-S",
        str(root / "sdk/cpp"),
        "-B",
        str(directory),
        f"-DCMAKE_BUILD_TYPE={configuration}",
        "-DHARMOS_BUILD_TESTS=ON",
    ]
    environment = dict(os.environ)
    # Git's Child Submodule Processes Need Long-Path Support in Windows Worktrees.
    count = int(environment.get("GIT_CONFIG_COUNT", "0"))
    environment["GIT_CONFIG_COUNT"] = str(count + 1)
    environment[f"GIT_CONFIG_KEY_{count}"] = "core.longpaths"
    environment[f"GIT_CONFIG_VALUE_{count}"] = "true"
    if os.name == "nt":
        configure += ["-G", "Visual Studio 17 2022", "-A", "x64"]

    # Configure the SDK and Fetch Its Immutable Dependency Graph.
    result = subprocess.run(
        configure, env=environment, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True, creationflags=spawn_flags()
    )
    sys.stdout.write(result.stdout)
    (directory / "configure.log").write_text(result.stdout, encoding="utf-8")
    result.check_returncode()

    if options.check:
        return

    if options.generate:
        for path in (directory / "generated").rglob("*.pb.*"):
            path.unlink()
    targets = ("harmos_cpp_protocol",) if options.generate else ("harmos-cpp-conformance", "harmos-cpp-tests")

    # Compile the Runtime and Code Generator with Bounded Parallelism.
    log = directory / "build.log"
    if log.exists():
        attempt = len(list(directory.glob("build-*.log"))) + 1
        shutil.copyfile(log, directory / f"build-{attempt}.log")
    print(f"C++ Build Log: {log}", flush=True)
    started = time.perf_counter()
    with log.open("w", encoding="utf-8") as output:
        result = subprocess.run(
            [
                "cmake",
                "--build",
                str(directory),
                "--config",
                configuration,
                "--target",
                *targets,
                "--parallel",
                "4",
            ],
            env=environment,
            stdout=output,
            stderr=subprocess.STDOUT,
            creationflags=spawn_flags(),
        )
    lines = log.read_text(encoding="utf-8", errors="replace").splitlines()
    if result.returncode:
        errors = [line for line in lines if ": error " in line or "fatal error" in line]
        print("\n".join(errors[:20] + lines[-20:]))
    else:
        print("\n".join(lines[-5:]))
    print(f"C++ Build: {time.perf_counter() - started:.1f} s; Complete Compiler Output: {log}", flush=True)
    result.check_returncode()

    if not options.generate:
        # Ship the Complete SDK Source Package, Independent of This Checkout's Tests.
        from .paths import output as build_output

        destination = build_output(root, "build", options.profile) / "cpp"
        destination.mkdir(parents=True, exist_ok=True)
        with tarfile.open(destination / "harmos-cpp.tar.gz", "w:gz") as package:
            for name in ("CMakeLists.txt", "include", "src", "cmake"):
                package.add(root / "sdk/cpp" / name, arcname=f"sdk/cpp/{name}")
            package.add(root / "proto", arcname="proto")


# === Private Functions === #

# === Entry Point === #

if __name__ == "__main__":
    main()
