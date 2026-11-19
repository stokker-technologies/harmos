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
import re
import shutil
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

# Third Party Imports
# Local Imports
from harmos_tools.build.cpp import environment
from harmos_tools.build.paths import output
from harmos_tools.process import spawn_flags

from tests.support import host

# Type Checking Imports


# === Public Exports === #

# === Constants === #

# === Types === #

# === Classes === #


class PackagedConformance(unittest.TestCase):
    def test_external_subproject_uses_installed_tools_and_opts_in_to_fixtures(self) -> None:
        # Install the Tools Wheel into an Isolated Interpreter.
        root = Path(__file__).resolve().parents[4]
        build = root / "target/cpp/subproject-consumer"
        build.mkdir(parents=True, exist_ok=True)
        wheel_directory = build / "wheel"
        subprocess.run(
            ["uv", "build", str(root / "tools/python"), "--wheel", "--out-dir", str(wheel_directory)],
            check=True,
            creationflags=spawn_flags(),
        )
        tools = build / "tools"
        subprocess.run(["uv", "venv", "--clear", "--python", sys.executable, str(tools)], check=True, creationflags=spawn_flags())
        python = tools / ("Scripts/python.exe" if os.name == "nt" else "bin/python")
        wheel = next(wheel_directory.glob("*.whl"))
        subprocess.run(["uv", "pip", "install", "--python", str(python), str(wheel)], check=True, creationflags=spawn_flags())
        child_environment = environment(root)
        child_environment.pop("PYTHONPATH", None)
        installed = subprocess.check_output(
            [str(python), "-c", "import harmos_tools.build.cpp; print(harmos_tools.build.cpp.__file__)"],
            text=True,
            env=child_environment,
            creationflags=spawn_flags(),
        ).strip()
        self.assertTrue(Path(installed).is_relative_to(tools), installed)

        # Add the SDK from a Consumer Outside the Library Tree.
        scratch = tempfile.TemporaryDirectory()
        self.addCleanup(scratch.cleanup)
        consumer = Path(scratch.name)
        self.assertFalse(consumer.is_relative_to(root))
        library = consumer / "library"
        profile = os.environ.get("TEST_PROFILE", "dev")
        package = output(root, "build", profile) / "cpp/harmos-cpp.tar.gz"
        shutil.unpack_archive(package, library, format="gztar", filter="data")
        (consumer / "CMakeLists.txt").write_text(
            "cmake_minimum_required(VERSION 3.24)\nproject(Consumer LANGUAGES C CXX)\n"
            f'add_subdirectory("{(library / "sdk/cpp").as_posix()}" harmos)\n'
            "if(NOT HARMOS_BUILD_TESTS)\n"
            "  foreach(target harmos_client_fixture harmos-cpp-client harmos-cpp-plugin harmos-cpp-tests harmos-cpp-conformance)\n"
            "    if(TARGET ${target})\n"
            '      message(FATAL_ERROR "Unrequested SDK Fixture: ${target}")\n'
            "    endif()\n  endforeach()\nendif()\n",
            encoding="utf-8",
        )
        toolchain = Path(child_environment["VCPKG_ROOT"]) / "scripts/buildsystems/vcpkg.cmake"
        configure = [
            "cmake",
            "-S",
            str(consumer),
            "-B",
            str(build / "native"),
            "-G",
            "Ninja",
            "--fresh",
            "-DCMAKE_BUILD_TYPE=Debug",
            f"-DCMAKE_TOOLCHAIN_FILE={toolchain}",
            f"-DVCPKG_INSTALLED_DIR={root / 'target/cpp/vcpkg_installed'}",
            "-DVCPKG_MANIFEST_INSTALL=OFF",
            f"-DPython3_EXECUTABLE={python}",
        ]
        result = subprocess.run(
            configure,
            cwd=consumer,
            env=child_environment,
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            creationflags=spawn_flags(),
        )
        self.assertEqual(result.returncode, 0, result.stdout)
        result = subprocess.run(
            ["cmake", "--build", str(build / "native"), "--parallel", os.environ.get("CARGO_BUILD_JOBS", "3")],
            cwd=consumer,
            env=child_environment,
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            creationflags=spawn_flags(),
        )
        (build / "default-build.log").write_text(result.stdout, encoding="utf-8")
        self.assertEqual(result.returncode, 0, result.stdout[-12000:])

        # Opt In and Build Only the SDK and Conformance Dependencies.
        result = subprocess.run(
            [*configure, "-DHARMOS_BUILD_TESTS=ON"],
            cwd=consumer,
            env=child_environment,
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            creationflags=spawn_flags(),
        )
        self.assertEqual(result.returncode, 0, result.stdout)
        result = subprocess.run(
            [
                "cmake",
                "--build",
                str(build / "native"),
                "--target",
                "harmos_cpp",
                "harmos-cpp-conformance",
                "--parallel",
                os.environ.get("CARGO_BUILD_JOBS", "3"),
            ],
            cwd=consumer,
            env=child_environment,
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            creationflags=spawn_flags(),
        )
        (build / "conformance-build.log").write_text(result.stdout, encoding="utf-8")
        self.assertEqual(result.returncode, 0, result.stdout[-12000:])
        suffix = ".exe" if os.name == "nt" else ""
        self.assertTrue((build / "native" / f"harmos-cpp-conformance{suffix}").is_file())
        for executable in (
            build / "native" / f"harmos-cpp-client{suffix}",
            build / "native" / f"harmos-cpp-plugin{suffix}",
            build / "native/harmos/tests" / f"harmos-cpp-tests{suffix}",
        ):
            self.assertFalse(executable.exists(), str(executable))

    def test_source_package_serves_the_public_fixture(self) -> None:
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
        shutil.unpack_archive(package, source, format="gztar", filter="data")
        # Compile the Packaged Example Against the Installed SDK, Without Its Sources.
        selected = "tests" if profile == "dev" else "release"
        prefix = root / "target/cpp" / selected / "installed"
        build = root / "target/cpp/packaged-consumer" / profile
        child_environment = environment(root)
        toolchain = Path(child_environment["VCPKG_ROOT"]) / "scripts/buildsystems/vcpkg.cmake"
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
            f"-DPython3_EXECUTABLE={sys.executable}",
        ]
        build.mkdir(parents=True, exist_ok=True)
        log = (build / "configure.log").open("w", encoding="utf-8")
        try:
            result = subprocess.run(
                configure,
                env=child_environment,
                check=False,
                stdout=log,
                stderr=subprocess.STDOUT,
                creationflags=spawn_flags(),
            )
        finally:
            log.close()
        self.assertEqual(result.returncode, 0, (build / "configure.log").read_text(errors="replace"))
        log = (build / "build.log").open("w", encoding="utf-8")
        try:
            result = subprocess.run(
                [
                    "cmake",
                    "--build",
                    str(build),
                    "--target",
                    "harmos-cpp-conformance",
                    "--parallel",
                    os.environ.get("CARGO_BUILD_JOBS", "3"),
                ],
                env=child_environment,
                check=False,
                stdout=log,
                stderr=subprocess.STDOUT,
                creationflags=spawn_flags(),
            )
        finally:
            log.close()
        self.assertEqual(
            result.returncode,
            0,
            (build / "build.log").read_text(errors="replace")[-12000:],
        )
        # Serve the Public Fixture and Run the Language-Neutral Host Through Every Rule.
        suffix = ".exe" if os.name == "nt" else ""
        plugin = build / f"harmos-cpp-conformance{suffix}"
        result = host.run_managed(plugin=plugin, timeout_s=120)
        self.assertEqual(result.returncode, 0, result.stdout)
        rows = [line for line in result.stdout.splitlines() if line.endswith("| PASS |")]
        summary = re.search(r"(\d+)/(\d+) Cases Passed", result.stdout)
        self.assertIsNotNone(summary, result.stdout)
        assert summary is not None
        self.assertEqual(int(summary[1]), int(summary[2]), result.stdout)
        self.assertEqual(len(rows), int(summary[2]), result.stdout)


# === Public Functions === #

# === Private Functions === #

# === Entry Point === #
