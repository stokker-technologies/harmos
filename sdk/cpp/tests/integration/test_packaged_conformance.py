# ***
#
# Harmos
#
# SPDX-License-Identifier: Apache-2.0
#
# ***


"""An Installed CMake SDK Package Serves Its Conformance Example"""

# === Imports === #

# Standard Library Imports
import os
import shutil
import subprocess
import tarfile
import unittest
from pathlib import Path

from harmos_tools.build.cpp import environment
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
                raise RuntimeError(
                    "Package Conformance: extracted sources escaped the owned directory"
                )
            if owned.exists():
                shutil.rmtree(owned)
        with tarfile.open(package) as bundle:
            bundle.extractall(source, filter="data")
        # Compile the Packaged Example Against the Installed SDK, Without Its Sources.
        selected = "tests" if profile == "dev" else "release"
        prefix = root / "target/cpp" / selected / "installed"
        build = root / "target/cpp/packaged" / profile
        child_environment = environment()
        toolchain = (
            Path(child_environment["VCPKG_ROOT"]) / "scripts/buildsystems/vcpkg.cmake"
        )
        configure = [
            "cmake",
            "-S",
            str(source / "sdk/cpp/examples/conformance"),
            "-B",
            str(build),
            "-G",
            "Ninja",
            f"-DCMAKE_BUILD_TYPE={configuration}",
            f"-DCMAKE_PREFIX_PATH={prefix}",
            f"-DCMAKE_TOOLCHAIN_FILE={toolchain}",
            f"-DVCPKG_INSTALLED_DIR={root / 'target/cpp/vcpkg_installed'}",
            "-DVCPKG_MANIFEST_INSTALL=OFF",
        ]
        subprocess.run(
            configure, env=child_environment, check=True, creationflags=spawn_flags()
        )
        build.mkdir(parents=True, exist_ok=True)
        with (build / "build.log").open("w", encoding="utf-8") as log:
            result = subprocess.run(
                [
                    "cmake",
                    "--build",
                    str(build),
                    "--target",
                    "harmos-cpp-conformance",
                    "--parallel",
                    "4",
                ],
                env=child_environment,
                check=False,
                stdout=log,
                stderr=subprocess.STDOUT,
                creationflags=spawn_flags(),
            )
        self.assertEqual(
            result.returncode,
            0,
            (build / "build.log").read_text(errors="replace")[-12000:],
        )
        # Serve the Public Fixture and Run the Language-Neutral Host Through Every Rule.
        suffix = ".exe" if os.name == "nt" else ""
        exported = output(root, "build", profile) / f"bin/harmos-conformance{suffix}"
        local = root / f"target/debug/harmos-conformance{suffix}"
        host = Path(
            os.environ.get(
                "HARMOS_CONFORMANCE_HOST", exported if exported.is_file() else local
            )
        )
        plugin = build / f"harmos-cpp-conformance{suffix}"
        result = subprocess.run(
            [str(host), "--plugin", str(plugin)],
            check=False,
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            text=True,
            timeout=120,
            creationflags=spawn_flags(),
        )
        self.assertEqual(result.returncode, 0, result.stdout)
        rows = [
            line for line in result.stdout.splitlines() if line.endswith("| PASS |")
        ]
        self.assertEqual(len(rows), 31, result.stdout)


# === Public Functions === #

# === Private Functions === #

# === Entry Point === #
