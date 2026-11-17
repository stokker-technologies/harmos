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
import json
import os
import shutil
import subprocess
import sys
import tarfile
import time
from argparse import ArgumentParser
from pathlib import Path
from subprocess import PIPE, STDOUT

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
    # Read Structured Input
    text = (ROOT / "sdk/cpp/vcpkg.json").read_text()
    manifest = json.loads(text)
    revision = manifest["builtin-baseline"]
    checkout = Path(cache) / "vcpkg" / revision[:12]
    environment["VCPKG_ROOT"] = str(checkout)

    # Bootstrap the Immutable Tool Checkout Outside the Repository.
    if not checkout.is_dir():
        checkout.parent.mkdir(parents=True, exist_ok=True)
        # Run the Native Command
        command = [
            "git",
            "clone",
            "--filter=blob:none",
            "--single-branch",
            "--branch",
            "2026.05.25",
            "https://github.com/microsoft/vcpkg.git",
            str(checkout),
        ]
        subprocess.run(
            command,
            check=True,
            stdout=sys.stdout,
            stderr=sys.stderr,
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
        subprocess.run(bootstrap, check=True, stdout=sys.stdout, stderr=sys.stderr, creationflags=spawn_flags())

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
        # Run the Native Command
        command = f'cmd /d /s /c ""{developer}" -no_logo -arch=x64 -host_arch=x64 >nul && set"'
        output = subprocess.check_output(
            command,
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
    parser = ArgumentParser(description=__doc__)
    parser.add_argument("--profile", choices=("dev", "release"), default="dev")
    parser.add_argument("--preset", choices=("development", "release", "tests"))
    parser.add_argument("--target", action="append", help="Build Only the Named CMake Target (Repeatable)")
    parser.add_argument("--prepare", action="store_true")
    parser.add_argument("--check", action="store_true")
    parser.add_argument("--format", action="store_true")
    parser.add_argument("--fresh", action="store_true")
    parser.add_argument("--generate", action="store_true")
    parser.add_argument("--test", action="store_true")
    parser.add_argument("--consumer", action="store_true")
    parser.add_argument("--layer", choices=("unit", "integration"))
    parser.add_argument("--exact")
    parser.add_argument("--verbose", action="store_true")
    parser.add_argument("--fixture-client", type=Path)
    parser.add_argument("--fixture-descriptors", type=Path)
    options = parser.parse_args()
    if options.fixture_client:
        from google.protobuf.descriptor_pb2 import FileDescriptorSet

        import harmos_tools.build.resources.cpp
        import harmos_tools.build.resources.descriptor

        if options.fixture_descriptors is None:
            parser.error("C++ fixture client requires --fixture-descriptors")
        library = FileDescriptorSet.FromString((ROOT / "crates/harmos-protocol/src/descriptors.bin").read_bytes())
        fixture = FileDescriptorSet.FromString(options.fixture_descriptors.read_bytes())
        methods = harmos_tools.build.resources.descriptor.table(library) + harmos_tools.build.resources.descriptor.table(fixture)
        harmos_tools.build.resources.cpp.client(
            ROOT, [method for method in methods if method["face"] == "client"], namespace="fixture", target=options.fixture_client
        )
        import harmos_tools.build.resources.cpp

        harmos_tools.build.resources.cpp.host(
            ROOT,
            [method for method in methods if method["face"] == "context"],
            namespace="fixture",
            target=options.fixture_client.with_name("plugin.hpp"),
        )
        return
    selected = options.preset or ("release" if options.profile == "release" else "tests")
    directory = ROOT / "target/cpp" / selected
    sdk = ROOT / "sdk/cpp"
    if options.format:
        sources = sorted(
            path
            for folder in ("src", "include", "examples", "tests")
            for path in (sdk / folder).rglob("*")
            if path.suffix in (".cpp", ".hpp")
        )
        arguments = ["clang-format", *(["--dry-run", "--Werror"] if options.check else ["-i"]), *map(str, sources)]
        subprocess.run(arguments, check=True, creationflags=spawn_flags())
        return
    child_environment = environment()
    if options.prepare:
        print(f"vcpkg: {child_environment['VCPKG_ROOT']}")
        return
    if options.generate:
        from google.protobuf.descriptor_pb2 import FileDescriptorSet

        import harmos_tools.build.resources.cpp
        import harmos_tools.build.resources.descriptor

        descriptors = FileDescriptorSet.FromString((ROOT / "crates/harmos-protocol/src/descriptors.bin").read_bytes())
        type_names = harmos_tools.build.resources.cpp.exports(ROOT, descriptors)
        harmos_tools.build.resources.cpp.client(
            ROOT,
            [method for method in harmos_tools.build.resources.descriptor.table(descriptors) if method["face"] == "client"],
            type_names=type_names,
        )
        harmos_tools.build.resources.cpp.host(
            ROOT,
            [method for method in harmos_tools.build.resources.descriptor.table(descriptors) if method["face"] == "context"],
            type_names=type_names,
        )

    # Tests Consume Existing Producer Outputs; a Selection That Runs No Test Fails.
    if options.test:
        if options.layer != "integration":
            arguments = ["ctest", "--preset", selected, "--no-tests=error"]
            if options.verbose:
                arguments.append("--verbose")
            if options.exact:
                import re

                arguments += ["-R", "^" + re.escape(options.exact) + "$"]
            subprocess.run(
                arguments, cwd=sdk, env=child_environment, check=True, stdout=sys.stdout, stderr=sys.stderr, creationflags=spawn_flags()
            )
        # An Exact Integration Selection Names One Python Story Module, Class or Test.
        integration = None
        if options.layer == "integration" and options.exact:
            integration = [options.exact]
        elif options.layer != "unit" and not options.exact:
            integration = ["discover", "-s", "tests/integration"]
        if integration is not None:
            # Run the Native Command
            command = [sys.executable, "-m", "unittest", *integration, "-v"]
            subprocess.run(
                command,
                cwd=sdk,
                env=dict(child_environment, TEST_PROFILE=options.profile),
                check=True,
                stdout=sys.stdout,
                stderr=sys.stderr,
                creationflags=spawn_flags(),
            )
        return

    # Build the Example Against Only the Installed Harmos Package.
    if options.consumer:
        consumer = ROOT / "target/cpp/consumer" / options.profile
        prefix = consumer / "installed"
        # Run the Native Command
        command = ["cmake", "--install", str(directory), "--prefix", str(prefix)]
        subprocess.run(
            command,
            env=child_environment,
            check=True,
            stdout=sys.stdout,
            stderr=sys.stderr,
            creationflags=spawn_flags(),
        )
        toolchain = Path(child_environment["VCPKG_ROOT"]) / "scripts/buildsystems/vcpkg.cmake"
        # Run the Native Command
        command = [
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
        ]
        subprocess.run(
            command,
            env=child_environment,
            check=True,
            stdout=sys.stdout,
            stderr=sys.stderr,
            creationflags=spawn_flags(),
        )
        # Run the Native Command
        command = ["cmake", "--build", str(consumer), "--parallel", "4"]
        subprocess.run(
            command,
            env=child_environment,
            check=True,
            stdout=sys.stdout,
            stderr=sys.stderr,
            creationflags=spawn_flags(),
        )
        return

    # Configure and Build the Shared Preset with Complete Compiler Evidence.
    directory.mkdir(parents=True, exist_ok=True)
    started = time.perf_counter()
    configure = ["cmake", "--preset", selected, f"-DPython3_EXECUTABLE={sys.executable}"]
    if options.fresh:
        configure.append("--fresh")
    result = subprocess.run(configure, cwd=sdk, env=child_environment, stdout=PIPE, stderr=STDOUT, text=True, creationflags=spawn_flags())
    sys.stdout.write(result.stdout)
    (directory / "configure.log").write_text(result.stdout, encoding="utf-8")
    result.check_returncode()
    if options.generate:
        for path in (directory / "generated").rglob("*.pb.*"):
            path.unlink()
    command = ["cmake", "--build", "--preset", selected, "--parallel", "4"]
    if options.generate:
        command += ["--target", "harmos_cpp_protocol"]
    elif options.target:
        command += ["--target", *options.target]
    log = directory / "build.log"
    if log.exists():
        attempt = len(list(directory.glob("build-*.log"))) + 1
        shutil.copyfile(log, directory / f"build-{attempt}.log")
    print(f"C++ Build Log: {log}", flush=True)
    output = log.open("w", encoding="utf-8")
    try:
        result = subprocess.run(command, cwd=sdk, env=child_environment, stdout=output, stderr=STDOUT, creationflags=spawn_flags())
    finally:
        output.close()
    lines = log.read_text(encoding="utf-8", errors="replace").splitlines()
    print("\n".join(lines[-40:] if result.returncode else lines[-5:]))
    print(f"C++ Configure and Build: {time.perf_counter() - started:.1f} s; Complete Compiler Output: {log}", flush=True)
    result.check_returncode()
    if options.check:
        # Visual Studio's Clang Frontend Checks the Native Compile Commands Too.
        if os.name == "nt":
            clang = Path(child_environment["VCINSTALLDIR"]) / "Tools/Llvm/bin/clang-tidy.exe"
            if not clang.is_file():
                raise RuntimeError("Check C++ Shadowing: install the Visual Studio C++ Clang tools component")
            # Read Structured Input
            text = (directory / "compile_commands.json").read_text(encoding="utf-8")
            commands = json.loads(text)
            # Translate the MSVC-Only Preprocessor Flag for the Clang Frontend.
            for command in commands:
                command["command"] = command["command"].replace(" /Zc:preprocessor", "")
            clang_directory = directory / "clang"
            clang_directory.mkdir(exist_ok=True)
            (clang_directory / "compile_commands.json").write_text(json.dumps(commands), encoding="utf-8")
            sources = sorted({entry["file"] for entry in commands if Path(entry["file"]).suffix == ".cpp"})
            # Run the Native Command
            command = [
                str(clang),
                "--checks=-*,clang-diagnostic-*,clang-analyzer-core.CallAndMessage",
                "--warnings-as-errors=*",
                "--extra-arg=-Wshadow",
                "--extra-arg=-Wshadow-all",
                "--extra-arg=-Werror=shadow",
                "--extra-arg=-Werror=shadow-all",
                "--extra-arg=-D_CRT_USE_BUILTIN_OFFSETOF",
                "-p",
                str(clang_directory),
                *sources,
            ]
            subprocess.run(
                command,
                env=child_environment,
                check=True,
                stdout=sys.stdout,
                stderr=sys.stderr,
                creationflags=spawn_flags(),
            )
        return

    # Install the SDK Before Any Compile-Contract Consumer Runs.
    if not options.generate:
        # Run the Native Command
        command = ["cmake", "--install", str(directory), "--prefix", str(directory / "installed")]
        subprocess.run(
            command,
            env=child_environment,
            check=True,
            stdout=sys.stdout,
            stderr=sys.stderr,
            creationflags=spawn_flags(),
        )

    # Stage the SDK Sources, Manifest and Conformance Consumer Together.
    if not options.generate:
        import harmos_tools.build.paths

        destination = harmos_tools.build.paths.output(ROOT, "build", options.profile) / "cpp"
        destination.mkdir(parents=True, exist_ok=True)
        package = tarfile.open(destination / "harmos-cpp.tar.gz", "w:gz")
        try:
            for name in ("CMakeLists.txt", "vcpkg.json", "include", "src", "cmake", "examples"):
                package.add(sdk / name, arcname=f"sdk/cpp/{name}")
            package.add(ROOT / "proto", arcname="proto")
            package.add(ROOT / "sdk/tests/fixtures/conformance", arcname="sdk/tests/fixtures/conformance")
            package.add(ROOT / "LICENSE", arcname="LICENSE")
        finally:
            package.close()


# === Private Functions === #

# === Entry Point === #

if __name__ == "__main__":
    main()
