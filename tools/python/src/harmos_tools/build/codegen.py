# ***
#
# Harmos
#
# SPDX-License-Identifier: Apache-2.0
#
# ***


"""Generate or Check Harmos's Own SDK Bindings Through the Harmos CLI."""

# === Imports === #

# Standard Library Imports
import os
import platform
import shutil
import subprocess
import sys
import xml.etree.ElementTree
from argparse import ArgumentParser
from dataclasses import dataclass
from pathlib import Path

# Third Party Imports
from google.protobuf.descriptor_pb2 import FileDescriptorSet

# Local Imports
from .. import temporary
from ..process import spawn_flags
from . import cargo

# Type Checking Imports


# === Public Exports === #

# === Constants === #

ROOT = Path(__file__).resolve().parents[5]
LANGUAGES = ("rust", "python", "csharp", "cpp")
LIBRARY = ROOT / "crates/harmos-protocol/src/descriptors.bin"
APPLICATION = ROOT / "sdk/rust/tests/fixtures/client/client-descriptors.bin"
CONFORMANCE = ROOT / "sdk/rust/tests/fixtures/conformance"
CPP_STYLE = "{BasedOnStyle: LLVM, ColumnLimit: 140, AllowShortFunctionsOnASingleLine: Empty, AllowShortLambdasOnASingleLine: None}"
WIRE = ("**/*_pb2.py", "**/*_pb2.pyi", "**/*_pb2_grpc.py")

# === Types === #

# === Classes === #


@dataclass(frozen=True)
class Placement:
    """Generated Files Below `source` Replace the Files `destination` Owns Through `owned`."""

    source: str
    destination: Path
    owned: tuple[str, ...]


PLACEMENTS = {
    "rust": (
        Placement(
            "sdk",
            ROOT / "sdk/rust/src",
            (
                "exports.rs",
                "client/types.rs",
                "client/resources.rs",
                "plugin/types.rs",
                "plugin/host.rs",
                "plugin/host_exports.rs",
                "context/contracts.rs",
            ),
        ),
        Placement("application", ROOT / "sdk/rust/tests/fixtures/client", ("resources.rs", "client_types.rs", "plugin.rs")),
    ),
    "python": (
        Placement("sdk", ROOT / "sdk/python/src/harmos/_generated", ("client.py", "host.py", *WIRE)),
        Placement("application", ROOT / "sdk/python/tests/fixtures/client/_generated", ("client.py", "plugin.py", *WIRE)),
        Placement("conformance", ROOT / "sdk/python/tests/fixtures/conformance", ("_caller.py",)),
        Placement("conformance/proto", ROOT / "sdk/python/tests/fixtures/conformance/_generated/proto", WIRE),
    ),
    "csharp": (
        Placement("sdk", ROOT / "sdk/csharp/src/Harmos/Generated", ("Client.g.cs", "Host.g.cs", "Wire/**/*.cs")),
        Placement("application", ROOT / "sdk/csharp/tests/Harmos.Tests/Generated", ("Client.g.cs", "Plugin.g.cs", "Wire/**/*.cs")),
        Placement("conformance", ROOT / "sdk/csharp/tests/Harmos.Conformance", ("CallerRequest.g.cs",)),
    ),
    "cpp": (
        Placement(
            "sdk",
            ROOT / "sdk/cpp",
            (
                "include/harmos/messages.hpp",
                "include/harmos/detail/messages.hpp",
                "include/harmos/_generated/*.hpp",
                "include/harmos/_generated/detail/*.hpp",
                "src/generated/*.cpp",
            ),
        ),
        Placement("conformance", ROOT / "sdk/cpp/examples/conformance", ("_wire/caller.hpp", "detail/types.hpp", "generated/types.hpp")),
    ),
}

# === Public Functions === #


def generate(languages: tuple[str, ...], *, check: bool = False) -> None:
    """Render Each Language Into Scratch Storage, Then Replace or Compare the Owned Files."""
    executable = cli()
    differences = []
    for language in languages:
        scratch_directory = temporary.Directory()
        scratch = scratch_directory.path
        try:
            _render(language, executable, scratch)
            for placement in PLACEMENTS[language]:
                differences += place(scratch / placement.source, placement, check=check)
        finally:
            scratch_directory.close()
    if differences:
        raise SystemExit(
            f"Check Generated Bindings: Run mise run generate -- --language {' --language '.join(languages)}\n" + "\n".join(differences)
        )


def cli() -> Path:
    """Build the Harmos CLI From This Checkout."""
    status = cargo.focused(ROOT, "build", ["--locked", "--profile", "dev", "-p", "harmos-cli"])
    if status:
        raise SystemExit(status)
    return ROOT / "target/debug" / ("harmos.exe" if os.name == "nt" else "harmos")


def place(source: Path, placement: Placement, *, check: bool) -> list[str]:
    """Replace the Owned Files, or Report How They Differ Without Writing."""
    destination = placement.destination
    expected = {path.relative_to(source) for pattern in placement.owned for path in source.glob(pattern) if path.is_file()}
    existing = {path.relative_to(destination) for pattern in placement.owned for path in destination.glob(pattern) if path.is_file()}
    if check:
        differences = [f"Missing: {(destination / path).relative_to(ROOT).as_posix()}" for path in sorted(expected - existing)]
        differences += [f"Obsolete: {(destination / path).relative_to(ROOT).as_posix()}" for path in sorted(existing - expected)]
        for path in sorted(expected & existing):
            if (source / path).read_bytes() != (destination / path).read_bytes():
                differences.append(f"Changed: {(destination / path).relative_to(ROOT).as_posix()}")
        return differences
    for path in existing - expected:
        (destination / path).unlink()
    for path in sorted(expected):
        target = destination / path
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(source / path, target)
    return []


def main() -> None:
    parser = ArgumentParser(description=__doc__)
    parser.add_argument("--language", choices=LANGUAGES, action="append")
    parser.add_argument("--check", action="store_true")
    options = parser.parse_args()
    languages = tuple(options.language or LANGUAGES)
    if "rust" in languages and not options.check:
        _protocols()
    generate(languages, check=options.check)


# === Private Functions === #


def _protocols() -> None:
    """Regenerate the Protocol Crate's Modules and Descriptors Before Any Binding Reads Them."""
    status = cargo.focused(ROOT, "build", ["--locked", "--profile", "dev", "-p", "harmos-protocol", "--example", "generate"])
    if status:
        raise SystemExit(status)
    command = [str(ROOT / "target/debug/examples" / ("generate.exe" if os.name == "nt" else "generate"))]
    subprocess.run(command, check=True, stdout=sys.stdout, stderr=sys.stderr, creationflags=spawn_flags())

    # The Payload Memory Module Leaves the Generator Unformatted
    command = ["rustfmt", "--edition", "2024", str(ROOT / "crates/harmos-domain/src/message/allocation/protobuf.rs")]
    subprocess.run(command, check=True, stdout=sys.stdout, stderr=sys.stderr, creationflags=spawn_flags())


def _render(language: str, executable: Path, scratch: Path) -> None:
    """Write One Language's Formatted Bindings Below `scratch`, One Folder per Placement Source."""
    request = (CONFORMANCE / "caller-request.bin").read_bytes()
    if language == "rust":
        # Generate the SDK Internals and the Application Fixture
        _codegen(executable, "rust", scratch / "sdk", "--descriptors", LIBRARY)
        _codegen(executable, "rust", scratch / "application", "--application", "--descriptors", LIBRARY, "--descriptors", APPLICATION)

        # Format With the SDK Policy
        sources = [str(path) for path in scratch.rglob("*.rs")]
        command = ["rustfmt", "--edition", "2024", "--config-path", str(ROOT / "sdk/rust/rustfmt.toml"), *sources]
        subprocess.run(command, check=True, stdout=sys.stdout, stderr=sys.stderr, creationflags=spawn_flags())
    elif language == "python":
        # Generate the SDK, the Application Fixture and the Conformance Wire
        protoc = ["--protoc", sys.executable, "--protoc-arg=-m", "--protoc-arg=grpc_tools.protoc"]
        _codegen(
            executable,
            "python",
            scratch / "sdk",
            "--descriptors",
            LIBRARY,
            *protoc,
            "--proto-path",
            ROOT / "proto",
        )
        _codegen(
            executable,
            "python",
            scratch / "application",
            "--application",
            "--module",
            "application=tests.fixtures.client._generated.proto",
            "--descriptors",
            LIBRARY,
            "--descriptors",
            APPLICATION,
            *protoc,
            "--proto-path",
            ROOT / "sdk/tests/fixtures/client/proto",
            "--proto-path",
            ROOT / "proto",
        )
        _codegen(
            executable,
            "python",
            scratch / "conformance",
            "--application",
            "--descriptors",
            CONFORMANCE / "descriptors.bin",
            *protoc,
            "--proto-path",
            ROOT / "sdk/tests/fixtures/conformance/proto",
        )
        caller = scratch / "conformance/_caller.py"
        caller.write_text(
            f"# @generated from caller probe descriptors; do not edit.\nREQUEST = {request!r}\n", encoding="utf-8", newline="\n"
        )

        # Sort Imports, Drop Unused Ones and Format With the SDK Policy
        sources = [str(path) for path in scratch.rglob("*.py") if "_pb2" not in path.name]
        style = str(ROOT / "sdk/python/pyproject.toml")
        command = [sys.executable, "-m", "ruff", "check", "--config", style, "--select", "I,F401", "--fix", "--quiet", *sources]
        subprocess.run(command, check=True, stdout=sys.stdout, stderr=sys.stderr, creationflags=spawn_flags())
        command = [sys.executable, "-m", "ruff", "format", "--config", style, "--quiet", *sources]
        subprocess.run(command, check=True, stdout=sys.stdout, stderr=sys.stderr, creationflags=spawn_flags())
    elif language == "csharp":
        # Generate the SDK, the Application Fixture and the Conformance Request
        tools = _csharp_tools()
        compiler = ["--protoc", tools / f"protoc{_suffix()}", "--grpc-plugin", tools / f"grpc_csharp_plugin{_suffix()}"]
        _codegen(executable, "csharp", scratch / "sdk", "--descriptors", LIBRARY, *compiler)
        _codegen(
            executable,
            "csharp",
            scratch / "application",
            "--application",
            "--csharp-namespace",
            "Fixture",
            "--descriptors",
            LIBRARY,
            "--descriptors",
            APPLICATION,
            *compiler,
        )
        target = scratch / "conformance/CallerRequest.g.cs"
        target.parent.mkdir(parents=True)
        target.write_text(
            "// @generated from caller probe descriptors; do not edit.\nnamespace Harmos.Conformance;\n\ninternal static class CallerRequest\n{\n"
            + "  internal static readonly byte[] Bytes = ["
            + ", ".join(str(byte) for byte in request)
            + "];\n}\n",
            encoding="utf-8",
            newline="\n",
        )
    elif language == "cpp":
        # Generate the SDK and the Conformance Request and Types
        _codegen(executable, "cpp", scratch / "sdk", "--descriptors", LIBRARY)
        caller = scratch / "conformance/_wire/caller.hpp"
        caller.parent.mkdir(parents=True)
        caller.write_text(
            "// @generated from caller probe descriptors; do not edit.\n#pragma once\n#include <array>\n"
            + "namespace harmos::conformance {\ninline constexpr std::array<unsigned char, "
            + str(len(request))
            + "> caller_request = {"
            + ", ".join(str(byte) for byte in request)
            + "};\n}\n",
            encoding="utf-8",
            newline="\n",
        )
        schema = FileDescriptorSet.FromString((CONFORMANCE / "descriptors.bin").read_bytes())
        lines = [
            "// @generated from protocol descriptors; do not edit.",
            "#pragma once",
            '#include "conformance/model/v1/fixture.pb.h"',
            "namespace harmos::conformance {",
        ]
        lines += [
            f"using ::{file.package.replace('.', '::')}::{message.name};"
            for file in schema.file
            if file.package == "conformance.model.v1"
            for message in file.message_type
        ]
        lines += ["}"]
        types = scratch / "conformance/detail/types.hpp"
        types.parent.mkdir(parents=True)
        types.write_text("\n".join(lines) + "\n", encoding="utf-8", newline="\n")
        facade = scratch / "conformance/generated/types.hpp"
        facade.parent.mkdir(parents=True)
        facade.write_text(
            '// @generated from protocol descriptors; do not edit.\n#pragma once\n#include "../detail/types.hpp"\n',
            encoding="utf-8",
            newline="\n",
        )

        # Format With the Generator Policy
        sources = [str(path) for path in scratch.rglob("*") if path.suffix in (".hpp", ".cpp")]
        command = ["clang-format", f"--style={CPP_STYLE}", "-i", *sources]
        subprocess.run(command, check=True, stdout=sys.stdout, stderr=sys.stderr, creationflags=spawn_flags())


def _codegen(executable: Path, language: str, out: Path, *arguments: str | Path) -> None:
    command = [str(executable), "codegen", language, "--out", str(out), *map(str, arguments)]
    subprocess.run(command, check=True, stdout=sys.stdout, stderr=sys.stderr, creationflags=spawn_flags())


def _csharp_tools() -> Path:
    """Select the SDK's Pinned Grpc.Tools Compiler From the Restored Package Cache."""
    properties = xml.etree.ElementTree.parse(ROOT / "sdk/csharp/Directory.Packages.props")
    versions = [item.attrib.get("Version") for item in properties.iter("PackageVersion") if item.attrib.get("Include") == "Grpc.Tools"]
    if len(versions) != 1 or not versions[0]:
        raise ValueError("Generate C# Bindings: Pin One Grpc.Tools Version in sdk/csharp/Directory.Packages.props")
    system = {"Windows": "windows", "Darwin": "macosx"}.get(platform.system(), "linux")
    architecture = "arm64" if platform.machine().lower() in ("arm64", "aarch64") else "x64"
    tools = Path.home() / ".nuget/packages/grpc.tools" / versions[0] / "tools" / f"{system}_{architecture}"
    if not tools.is_dir():
        raise FileNotFoundError("Generate C# Bindings: Restore the Pinned Grpc.Tools Package With mise -C sdk/csharp run restore")
    return tools


def _suffix() -> str:
    return ".exe" if os.name == "nt" else ""


# === Entry Point === #

if __name__ == "__main__":
    main()
