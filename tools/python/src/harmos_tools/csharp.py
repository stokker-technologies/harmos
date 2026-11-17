# ***
#
# Harmos
#
# SPDX-License-Identifier: Apache-2.0
#
# ***

"""Build and Prove the C# Plugin SDK Through the Shared Reference Host."""

# === Imports === #

# Standard Library Imports
import subprocess
import xml.etree.ElementTree
from argparse import ArgumentParser
from pathlib import Path

# Third Party Imports
# Local Imports
from .process import spawn_flags

# Type Checking Imports

# === Public Exports === #

# === Constants === #

# === Types === #

# === Classes === #

# === Public Functions === #


def executable(root: Path, profile: str = "dev") -> Path:
    configuration = "Release" if profile == "release" else "Debug"
    manifest = root / "sdk/csharp/artifacts/paths" / configuration / "Harmos.Conformance.txt"
    if not manifest.is_file():
        raise FileNotFoundError("Build the C# Fixture: mise run build -- --language csharp")
    return root / "sdk/csharp" / manifest.read_text(encoding="utf-8-sig").strip()


def protocols(
    root: Path,
    *,
    descriptors: Path | None = None,
    destination: Path | None = None,
    check: bool = False,
    package: str | None = None,
    files: set[str] | None = None,
    namespace_override: str | None = None,
) -> None:
    """Keep Wire Requests and Carriers Internal; Give Payloads Their Public Namespace."""

    descriptors = descriptors or root / "crates/harmos-protocol/src/descriptors.bin"
    # Select the SDK's Pinned Protocol Compiler
    properties = xml.etree.ElementTree.parse(root / "sdk/csharp/Directory.Packages.props")
    versions = [item.attrib.get("Version") for item in properties.iter("PackageVersion") if item.attrib.get("Include") == "Grpc.Tools"]
    version = versions[0] if len(versions) == 1 else None
    if not version:
        raise ValueError("Generate C# Protocol: Pin one Grpc.Tools version in sdk/csharp/Directory.Packages.props")
    tools = Path.home() / ".nuget/packages/grpc.tools" / version / "tools"
    from harmos_tools.build import csharp_protocol

    csharp_protocol.protocols(
        root,
        descriptors=descriptors,
        namespace=None,
        tools=tools,
        destination=destination,
        check=check,
        package=package,
        files=files,
        namespace_override=namespace_override,
    )


def clients(root: Path, *, check: bool = False) -> None:
    from google.protobuf.descriptor_pb2 import FileDescriptorSet

    import harmos_tools.build.resource_roots
    import harmos_tools.build.resources.csharp
    import harmos_tools.build.resources.descriptor

    descriptors = FileDescriptorSet.FromString((root / "crates/harmos-protocol/src/descriptors.bin").read_bytes())
    methods = [method for method in harmos_tools.build.resources.descriptor.table(descriptors) if method["face"] == "client"]
    protocols(root, check=check)
    protocols(
        root,
        destination=root / "sdk/csharp/tests/Harmos.Conformance/Generated",
        check=check,
        files={"harmos/host/v1/invocations.proto"},
        namespace_override="Harmos.Conformance.Wire",
    )
    fixture_path = root / "sdk/rust/tests/fixtures/client/client-descriptors.bin"
    protocols(
        root,
        descriptors=fixture_path,
        destination=root / "sdk/csharp/tests/Harmos.Tests/Generated/Wire",
        check=check,
        package="application",
    )
    harmos_tools.build.resources.csharp.client(root, methods, check=check)
    harmos_tools.build.resources.csharp.host(
        root, [method for method in harmos_tools.build.resources.descriptor.table(descriptors) if method["face"] == "context"], check=check
    )
    harmos_tools.build.resource_roots.compose(
        root,
        [fixture_path],
        root / "sdk/csharp/tests/Harmos.Tests/Generated/Client.g.cs",
        "fixture",
        language="csharp",
        composition=root / "sdk/tests/fixtures/client/composition.toml",
        check=check,
    )


def run(root: Path, command: str, profile: str = "dev", *, check: bool = False, layer: str | None = None) -> int:
    if command in ("generate", "check-generated"):
        clients(root, check=command == "check-generated")
        return 0
    arguments = ["mise", "-C", str(root / "sdk/csharp"), "run", command, "--"]
    if command in ("build", "test", "package"):
        arguments += ["--profile", profile]
    if check:
        arguments += ["--check"]
    if layer:
        arguments += ["--layer", layer]
    return subprocess.run(arguments, cwd=root, check=False, creationflags=spawn_flags()).returncode


def main() -> None:
    parser = ArgumentParser(description=__doc__)
    parser.add_argument("command", choices=("build", "test", "format", "check", "package", "generate", "check-generated"))
    parser.add_argument("--profile", choices=("dev", "release"), default="dev")
    parser.add_argument("--check", action="store_true")
    parser.add_argument("--layer", choices=("unit", "integration"))
    options = parser.parse_args()
    raise SystemExit(run(Path(__file__).resolve().parents[4], options.command, options.profile, check=options.check, layer=options.layer))


# === Private Functions === #

# === Entry Point === #

if __name__ == "__main__":
    main()
