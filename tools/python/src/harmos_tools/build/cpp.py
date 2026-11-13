# ***
#
# Harmos
#
# SPDX-License-Identifier: Apache-2.0
#
# ***

"""Build the C++ SDK Through Its CMake Presets and Pinned vcpkg Packages."""

# === Imports === #

# Standard Library Imports
import argparse
import json
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

ROOT = Path(__file__).resolve().parents[5]

# === Types === #

# === Classes === #

# === Public Functions === #


def environment() -> dict[str, str]:
    environment = dict(os.environ)
    cache = subprocess.check_output(["mise", "cache", "path"], text=True, creationflags=spawn_flags()).strip()
    manifest = json.loads((ROOT / "sdk/cpp/vcpkg.json").read_text())
    revision = manifest["builtin-baseline"]
    checkout = Path(cache) / "vcpkg" / revision[:12]
    environment["VCPKG_ROOT"] = str(checkout)

    # Bootstrap the Immutable Tool Checkout Outside the Repository.
    if not checkout.is_dir():
        checkout.parent.mkdir(parents=True, exist_ok=True)
        subprocess.run(
            [
                "git",
                "clone",
                "--filter=blob:none",
                "--single-branch",
                "--branch",
                "2026.05.25",
                "https://github.com/microsoft/vcpkg.git",
                str(checkout),
            ],
            check=True,
            creationflags=spawn_flags(),
        )
    actual = subprocess.check_output(["git", "-C", str(checkout), "rev-parse", "HEAD"], text=True, creationflags=spawn_flags()).strip()
    if actual != revision:
        raise RuntimeError(f"vcpkg Checkout: Expected {revision}, Got {actual}")
    executable = checkout / ("vcpkg.exe" if os.name == "nt" else "vcpkg")
    if not executable.is_file():
        bootstrap = (
            ["cmd", "/c", str(checkout / "bootstrap-vcpkg.bat"), "-disableMetrics"]
            if os.name == "nt"
            else ["sh", str(checkout / "bootstrap-vcpkg.sh"), "-disableMetrics"]
        )
        subprocess.run(bootstrap, check=True, creationflags=spawn_flags())

    # Ninja Uses the Native MSVC Environment on Windows.
    if os.name == "nt" and not environment.get("VCINSTALLDIR"):
        tool_path = environment["PATH"]
        vswhere = Path(environment["PROGRAMFILES(X86)"]) / "Microsoft Visual Studio/Installer/vswhere.exe"
        installation = subprocess.check_output(
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
        developer = Path(installation) / "Common7/Tools/VsDevCmd.bat"
        output = subprocess.check_output(
            f'cmd /d /s /c ""{developer}" -no_logo -arch=x64 -host_arch=x64 >nul && set"',
            text=True,
            env=dict(environment, PATH=str(vswhere.parent) + os.pathsep + environment["PATH"]),
            creationflags=spawn_flags(),
        )
        environment.update(
            (key.upper(), value)
            for line in output.splitlines()
            if "=" in line and not line.startswith("=")
            for key, value in [line.split("=", 1)]
        )
        environment["PATH"] = tool_path + os.pathsep + environment["PATH"]
    environment["VCPKG_ROOT"] = str(checkout)
    return environment


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--profile", choices=("dev", "release"), default="dev")
    parser.add_argument("--preset", choices=("development", "release", "tests"))
    parser.add_argument("--prepare", action="store_true")
    parser.add_argument("--check", action="store_true")
    parser.add_argument("--generate", action="store_true")
    parser.add_argument("--test", action="store_true")
    parser.add_argument("--consumer", action="store_true")
    parser.add_argument("--layer", choices=("unit", "integration"))
    parser.add_argument("--exact")
    options = parser.parse_args()
    selected = options.preset or ("release" if options.profile == "release" else "tests")
    directory = ROOT / "target/cpp" / selected
    sdk = ROOT / "sdk/cpp"
    child_environment = environment()
    if options.prepare:
        print(f"vcpkg: {child_environment['VCPKG_ROOT']}")
        return
    if options.generate:
        from google.protobuf.descriptor_pb2 import FileDescriptorSet

        from .resource_roots import cpp, table

        descriptors = FileDescriptorSet.FromString((ROOT / "crates/harmos-protocol/src/descriptors.bin").read_bytes())
        cpp(ROOT, [method for method in table(descriptors) if method["face"] == "client"])

    # Tests Consume Existing Producer Outputs.
    if options.test:
        if options.layer != "integration":
            arguments = ["ctest", "--preset", selected]
            if options.exact:
                import re

                arguments += ["-R", "^" + re.escape(options.exact) + "$"]
            subprocess.run(arguments, cwd=sdk, env=child_environment, check=True, creationflags=spawn_flags())
        if options.layer != "unit" and not options.exact:
            subprocess.run(
                [sys.executable, "-m", "unittest", "discover", "-s", "tests/integration", "-v"],
                cwd=sdk,
                env=dict(child_environment, TEST_PROFILE=options.profile),
                check=True,
                creationflags=spawn_flags(),
            )
        return

    # Build the Example Against Only the Installed Harmos Package.
    if options.consumer:
        prefix = directory / "installed"
        subprocess.run(
            ["cmake", "--install", str(directory), "--prefix", str(prefix)], env=child_environment, check=True, creationflags=spawn_flags()
        )
        consumer = ROOT / "target/cpp/consumer" / options.profile
        toolchain = Path(child_environment["VCPKG_ROOT"]) / "scripts/buildsystems/vcpkg.cmake"
        subprocess.run(
            [
                "cmake",
                "-S",
                str(sdk / "examples/conformance"),
                "-B",
                str(consumer),
                "-G",
                "Ninja",
                f"-DCMAKE_TOOLCHAIN_FILE={toolchain}",
                f"-DCMAKE_PREFIX_PATH={prefix}",
                f"-DVCPKG_INSTALLED_DIR={ROOT / 'target/cpp/vcpkg_installed'}",
                "-DVCPKG_MANIFEST_INSTALL=OFF",
                f"-DCMAKE_BUILD_TYPE={'Debug' if options.profile == 'dev' else 'Release'}",
            ],
            env=child_environment,
            check=True,
            creationflags=spawn_flags(),
        )
        subprocess.run(
            ["cmake", "--build", str(consumer), "--parallel", "4"], env=child_environment, check=True, creationflags=spawn_flags()
        )
        return

    # Configure and Build the Shared Preset with Complete Compiler Evidence.
    directory.mkdir(parents=True, exist_ok=True)
    started = time.perf_counter()
    configure = ["cmake", "--preset", selected, f"-DPython3_EXECUTABLE={sys.executable}"]
    result = subprocess.run(
        configure, cwd=sdk, env=child_environment, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True, creationflags=spawn_flags()
    )
    sys.stdout.write(result.stdout)
    (directory / "configure.log").write_text(result.stdout, encoding="utf-8")
    result.check_returncode()
    if options.check:
        return
    if options.generate:
        for path in (directory / "generated").rglob("*.pb.*"):
            path.unlink()
    command = ["cmake", "--build", "--preset", selected, "--parallel", "4"]
    if options.generate:
        command += ["--target", "harmos_cpp_protocol"]
    log = directory / "build.log"
    if log.exists():
        attempt = len(list(directory.glob("build-*.log"))) + 1
        shutil.copyfile(log, directory / f"build-{attempt}.log")
    print(f"C++ Build Log: {log}", flush=True)
    with log.open("w", encoding="utf-8") as output:
        result = subprocess.run(
            command, cwd=sdk, env=child_environment, stdout=output, stderr=subprocess.STDOUT, creationflags=spawn_flags()
        )
    lines = log.read_text(encoding="utf-8", errors="replace").splitlines()
    print("\n".join(lines[-40:] if result.returncode else lines[-5:]))
    print(f"C++ Configure and Build: {time.perf_counter() - started:.1f} s; Complete Compiler Output: {log}", flush=True)
    result.check_returncode()

    # Install the SDK Before Any Compile-Contract Consumer Runs.
    if not options.generate:
        subprocess.run(
            ["cmake", "--install", str(directory), "--prefix", str(directory / "installed")],
            env=child_environment,
            check=True,
            creationflags=spawn_flags(),
        )

    # Stage the SDK Sources, Manifest and Conformance Consumer Together.
    if not options.generate:
        from .paths import output as build_output

        destination = build_output(ROOT, "build", options.profile) / "cpp"
        destination.mkdir(parents=True, exist_ok=True)
        with tarfile.open(destination / "harmos-cpp.tar.gz", "w:gz") as package:
            for name in ("CMakeLists.txt", "vcpkg.json", "include", "src", "cmake", "examples"):
                package.add(sdk / name, arcname=f"sdk/cpp/{name}")
            package.add(ROOT / "proto", arcname="proto")
            package.add(ROOT / "LICENSE", arcname="LICENSE")


# === Private Functions === #

# === Entry Point === #

if __name__ == "__main__":
    main()
