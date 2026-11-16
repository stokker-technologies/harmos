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
import platform
import re
import shutil
import subprocess
import tempfile
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
    from google.protobuf.descriptor_pb2 import FileDescriptorSet

    import harmos_tools.build.resources.csharp
    import harmos_tools.build.resources.descriptor

    descriptors = descriptors or root / "crates/harmos-protocol/src/descriptors.bin"
    schema = FileDescriptorSet.FromString(descriptors.read_bytes())
    requests = {method["request"].rsplit(".", 1)[-1] for method in harmos_tools.build.resources.descriptor.table(schema)}
    requests.update(
        {"Request", "Response", "ChildRequest", "ChildResponse", "ChildCancel", "ChildWatch", "ChildWatchResponse", "ChildWatchCancel"}
    )
    # Select the SDK's Pinned Protocol Compiler
    properties = xml.etree.ElementTree.parse(root / "sdk/csharp/Directory.Packages.props")
    versions = [item.attrib.get("Version") for item in properties.iter("PackageVersion") if item.attrib.get("Include") == "Grpc.Tools"]
    version = versions[0] if len(versions) == 1 else None
    if not version:
        raise ValueError("Generate C# Protocol: Pin one Grpc.Tools version in sdk/csharp/Directory.Packages.props")
    tools = Path.home() / ".nuget/packages/grpc.tools" / version / "tools"
    systems = {"Windows": "windows", "Darwin": "macosx"}
    system = systems.get(platform.system(), "linux")
    architecture = "arm64" if platform.machine().lower() in ("arm64", "aarch64") else "x64"
    executable_suffix = ".exe" if system == "windows" else ""
    protoc = tools / f"{system}_{architecture}" / ("protoc" + executable_suffix)
    plugin = protoc.with_name("grpc_csharp_plugin" + executable_suffix)
    if not protoc.is_file():
        raise FileNotFoundError("Generate C# Protocol: Run mise -C sdk/csharp run restore")
    scratch = Path(tempfile.mkdtemp())
    destination = destination or root / "sdk/csharp/src/Harmos/Generated/Wire"
    try:
        for file in schema.file:
            if file.name.startswith("google/") or file.name.endswith("guest.proto"):
                continue
            if package is not None and not (file.package == package or file.package.startswith(package + ".")):
                continue
            if files is not None and file.name not in files:
                continue
            output = scratch / Path(file.name).with_suffix("")
            output.mkdir(parents=True, exist_ok=True)
            command = [str(protoc), f"--descriptor_set_in={descriptors}", f"--csharp_out={output}"]
            if file.service and namespace_override is None:
                command += [f"--plugin=protoc-gen-grpc={plugin}", f"--grpc_out={output}", "--grpc_opt=internal_access"]
            # Run the Native Command
            command = [*command, file.name]
            subprocess.run(command, check=True, creationflags=spawn_flags())
            for path in output.glob("*.cs"):
                source = path.read_text(encoding="utf-8")
                source = harmos_tools.build.resources.csharp.message(source)
                if namespace_override is not None:
                    source = source.replace("namespace Harmos.Clients {", f"namespace {namespace_override} {{")
                    source = source.replace("global::Harmos.Clients.", f"global::{namespace_override}.")
                for name in requests:
                    source = source.replace(f"public sealed partial class {name} :", f"internal sealed partial class {name} :")
                if path.name.endswith("Grpc.cs"):
                    source = source.replace("namespace Harmos.Plugins {", "namespace Harmos.Internal.Host {")
                    source = source.replace("namespace Harmos.Clients {", "namespace Harmos.Internal.Client {")
                    namespace = harmos_tools.build.resources.csharp.message(file.options.csharp_namespace)
                    if namespace.endswith(".Clients"):
                        source = source.replace(
                            f"namespace {namespace} {{", f"namespace {namespace.removesuffix('.Clients')}.Internal.Client {{"
                        )
                # Carrier Messages Are Never an Authoring Surface.
                if file.package == "harmos.plugin.v1":
                    source = source.replace("public sealed partial class ", "internal sealed partial class ")
                # Generated Namespaces Follow the Same File-Scoped Rule as Authored C#.
                match = re.search(r"namespace ([\w.]+) \{", source)
                if match:
                    source = source[: match.start()] + f"namespace {match[1]};" + source[match.end() :]
                    ending = source.rfind("}")
                    source = source[:ending] + source[ending + 1 :]
                path.write_text(source, encoding="utf-8", newline="\n")
        if check:
            expected = {path.relative_to(scratch) for path in scratch.rglob("*.cs")}
            existing = {path.relative_to(destination) for path in destination.rglob("*.cs")}
            if expected != existing or any((scratch / path).read_bytes() != (destination / path).read_bytes() for path in expected):
                raise ValueError("Check C# Protocol: Generated Drift; Run mise run generate -- --sdk csharp")
        else:
            if destination.exists():
                shutil.rmtree(destination)
            shutil.copytree(scratch, destination)
    finally:
        shutil.rmtree(scratch)


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
