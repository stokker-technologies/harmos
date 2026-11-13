# ***
#
# Harmos
#
# SPDX-License-Identifier: Apache-2.0
#
# ***


"""A Standalone CMake SDK Package Serves Its Public Conformance Fixture"""

# === Imports === #

# Standard Library Imports
import os
import shutil
import subprocess
import tarfile
import unittest
from pathlib import Path

from harmos_tools.build.paths import output
from harmos_tools.process import spawn_flags

# Third Party Imports

# Local Imports

# Type Checking Imports


# === Public Exports === #

# === Constants === #

# === Types === #

# === Classes === #


class PackagedConformance(unittest.TestCase):
    def test_source_package_serves_the_public_fixture(self):
        root = Path(__file__).resolve().parents[4]
        profile = os.environ.get("TEST_PROFILE", "dev")
        configuration = "Debug" if profile == "dev" else "Release"
        source = (root / "target/generated/cpp/packaged-conformance").resolve()
        source.mkdir(parents=True, exist_ok=True)
        package = output(root, "build", profile) / "cpp/harmos-cpp.tar.gz"
        # Extract Only the Shipping Source Package into the Consumer's Directory.
        for name in ("sdk", "proto"):
            owned = (source / name).resolve()
            if not owned.is_relative_to(source):
                raise RuntimeError("Package Conformance: extracted sources escaped the owned directory")
            if owned.exists():
                shutil.rmtree(owned)
        with tarfile.open(package) as bundle:
            bundle.extractall(source, filter="data")
        (source / "main.cpp").write_text(
            '#include "harmos/conformance.hpp"\nint main() { harmos::conformance::plugin().serve(); }\n', encoding="utf-8"
        )
        (source / "CMakeLists.txt").write_text(
            "cmake_minimum_required(VERSION 3.24)\nproject(PackagedConformance LANGUAGES C CXX)\nadd_subdirectory(sdk/cpp)\nadd_executable(packaged-cpp-conformance main.cpp)\ntarget_link_libraries(packaged-cpp-conformance PRIVATE Harmos::Cpp)\n",
            encoding="utf-8",
        )
        build = root / "target/cpp/packaged" / profile
        cache = (root / "target/cpp" / profile / "CMakeCache.txt").read_text()
        dependency = Path(next(line.split("=", 1)[1] for line in cache.splitlines() if line.startswith("grpc_SOURCE_DIR:STATIC=")))
        configure = [
            "cmake",
            "-S",
            str(source),
            "-B",
            str(build),
            f"-DCMAKE_BUILD_TYPE={configuration}",
            "-DHARMOS_BUILD_TESTS=OFF",
            f"-DFETCHCONTENT_SOURCE_DIR_GRPC={dependency}",
        ]
        if os.name == "nt":
            configure += ["-G", "Visual Studio 17 2022", "-A", "x64"]
        # Build the Package with Only Its Published Headers and Ordinary gRPC Dependencies.
        subprocess.run(configure, check=True, creationflags=spawn_flags())
        with (build / "build.log").open("w", encoding="utf-8") as log:
            result = subprocess.run(
                ["cmake", "--build", str(build), "--config", configuration, "--target", "packaged-cpp-conformance", "--parallel", "4"],
                stdout=log,
                stderr=subprocess.STDOUT,
                creationflags=spawn_flags(),
            )
        self.assertEqual(result.returncode, 0, (build / "build.log").read_text(errors="replace")[-12000:])
        # Serve the Public Fixture and Run the Language-Neutral Host Through Every Rule.
        suffix = ".exe" if os.name == "nt" else ""
        exported = output(root, "build", profile) / f"bin/harmos-conformance{suffix}"
        local = root / f"target/debug/harmos-conformance{suffix}"
        host = Path(os.environ.get("HARMOS_CONFORMANCE_HOST", exported if exported.is_file() else local))
        plugin = build / configuration / f"packaged-cpp-conformance{suffix}" if os.name == "nt" else build / "packaged-cpp-conformance"
        result = subprocess.run(
            [str(host), "--plugin", str(plugin)],
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            text=True,
            timeout=120,
            creationflags=spawn_flags(),
        )
        self.assertEqual(result.returncode, 0, result.stdout)


# === Public Functions === #

# === Private Functions === #

# === Entry Point === #
