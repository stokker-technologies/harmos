# ***
#
# Harmos
#
# SPDX-License-Identifier: Apache-2.0
#
# ***


"""Generate Root Resource Contracts."""

# === Imports === #

# Standard Library Imports
import re
import subprocess
import sys
import tomllib
from argparse import ArgumentParser
from pathlib import Path

# Third Party Imports
from google.protobuf.descriptor_pb2 import FileDescriptorSet

# Local Imports
import harmos_tools.build.resources.cpp
import harmos_tools.build.resources.csharp
import harmos_tools.build.resources.descriptor
import harmos_tools.build.resources.python
import harmos_tools.build.resources.rust
from harmos_tools.process import spawn_flags
from harmos_tools.source.protocol import narrow_callers

# Type Checking Imports

# === Public Exports === #

__all__ = ["compose", "generate", "main"]

# === Constants === #

# === Types === #

# === Classes === #

# === Public Functions === #


def generate(root: Path, descriptors: FileDescriptorSet | None = None, *, namespace: str = "harmos") -> None:
    if descriptors is None:
        descriptors = FileDescriptorSet.FromString((root / "crates/harmos-protocol/src/descriptors.bin").read_bytes())
    for folder in [
        "sdk",
        "sdk/python/src/harmos",
        "sdk/rust/src/connection",
        "sdk/csharp/src/Harmos/Generated",
        "sdk/cpp/include/harmos/_generated",
    ]:
        (root / folder).mkdir(parents=True, exist_ok=True)
    methods = harmos_tools.build.resources.descriptor.table(descriptors)
    type_names = harmos_tools.build.resources.cpp.exports(root, descriptors)
    harmos_tools.build.resources.rust.exports(root, descriptors)
    harmos_tools.build.resources.python.exports(root / "sdk/python/src/harmos/_generated", descriptors)
    conformance = root / "sdk/rust/tests/fixtures/conformance/descriptors.bin"
    if conformance.is_file():
        request = conformance.with_name("caller-request.bin").read_bytes()
        (root / "sdk/python/tests/fixtures/conformance/_caller.py").write_text(
            f"# @generated from caller probe descriptors; do not edit.\nREQUEST = {request!r}\n", encoding="utf-8", newline="\n"
        )
        # Run the Native Command
        command = [sys.executable, "-m", "ruff", "format", str(root / "sdk/python/tests/fixtures/conformance/_caller.py")]
        subprocess.run(
            command,
            check=True,
            creationflags=spawn_flags(),
        )
        (root / "sdk/csharp/tests/Harmos.Conformance/CallerRequest.g.cs").write_text(
            "// @generated from caller probe descriptors; do not edit.\nnamespace Harmos.Conformance;\n\ninternal static class CallerRequest\n{\n"
            + "  internal static readonly byte[] Bytes = ["
            + ", ".join(str(byte) for byte in request)
            + "];\n}\n",
            encoding="utf-8",
            newline="\n",
        )
        caller = root / "sdk/cpp/examples/conformance/_wire/caller.hpp"
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
        harmos_tools.build.resources.cpp.format_code([caller])
        schema = FileDescriptorSet.FromString(conformance.read_bytes())
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
        target = root / "sdk/cpp/examples/conformance/detail/types.hpp"
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_text("\n".join(lines) + "\n", encoding="utf-8", newline="\n")
        facade = target.parent.parent / "generated/types.hpp"
        facade.parent.mkdir(parents=True, exist_ok=True)
        facade.write_text(
            '// @generated from protocol descriptors; do not edit.\n#pragma once\n#include "../detail/types.hpp"\n',
            encoding="utf-8",
            newline="\n",
        )
        harmos_tools.build.resources.cpp.format_code([target, facade])
    clients = [method for method in methods if method["face"] == "client"]
    harmos_tools.build.resources.python.client(root / "sdk/python/src/harmos/_generated/client.py", clients, descriptors)
    if namespace == "harmos":
        harmos_tools.build.resources.python.host(
            root / "sdk/python/src/harmos/_generated/host.py", [method for method in methods if method["face"] == "context"], descriptors
        )
    harmos_tools.build.resources.rust.client(root, clients)
    if namespace == "harmos":
        harmos_tools.build.resources.rust.context(root, [method for method in methods if method["face"] == "context"])
        harmos_tools.build.resources.rust.host(root, [method for method in methods if method["face"] == "context"])
    harmos_tools.build.resources.csharp.client(root, clients, namespace=namespace)
    if namespace == "harmos":
        harmos_tools.build.resources.csharp.host(root, [method for method in methods if method["face"] == "context"])
    harmos_tools.build.resources.cpp.client(root, clients, namespace=namespace, type_names=type_names)
    if namespace == "harmos":
        harmos_tools.build.resources.cpp.host(root, [method for method in methods if method["face"] == "context"], type_names=type_names)
    fixture = root / "sdk/rust/tests/fixtures/client/client-descriptors.bin"
    if fixture.is_file():
        for language, target, fixture_namespace in [
            ("python", root / "sdk/python/tests/fixtures/client/_generated/client.py", "tests.fixtures.client"),
            ("rust", fixture.parent / "resources.rs", "fixture"),
            ("csharp", root / "sdk/csharp/tests/Harmos.Tests/Generated/Client.g.cs", "fixture"),
        ]:
            compose(
                root,
                [fixture],
                target,
                fixture_namespace,
                language=language,
                composition=root / "sdk/tests/fixtures/client/composition.toml",
            )
    paths = [
        root / "sdk/rust/src" / name
        for name in (
            "exports.rs",
            "client/types.rs",
            "client/resources.rs",
            "connection/resources.rs",
            "context/contracts.rs",
            "plugin/host.rs",
            "plugin/host_exports.rs",
            "plugin/types.rs",
        )
    ]
    paths = [path for path in paths if path.is_file()]
    if fixture.is_file():
        paths.extend([fixture.parent / "resources.rs", fixture.parent / "client_types.rs", fixture.parent / "plugin.rs"])
    # Run the Native Command
    command = ["rustfmt", "--edition", "2024", "--config-path", str(root / "sdk/rust/rustfmt.toml"), *map(str, paths)]
    subprocess.run(
        command,
        check=True,
        creationflags=spawn_flags(),
    )


def compose(
    repository: Path,
    descriptors: list[Path],
    output: Path,
    namespace: str,
    *,
    language: str | None = None,
    composition: Path | None = None,
    check: bool = False,
) -> None:
    if not re.fullmatch(r"[a-z][a-z0-9_]*(?:\.[a-z][a-z0-9_]*)*", namespace):
        raise ValueError("Generate Client: Invalid Output Namespace; Correct the composition inputs and rerun mise run generate")
    merged = FileDescriptorSet()
    files = {}
    libraries = []
    settings = {}
    if composition is not None:
        # Read Structured Input
        text = composition.read_text(encoding="utf-8")
        settings = tomllib.loads(text)
        if set(settings) - {"libraries", "callers"}:
            raise ValueError("Compose SDK: Unknown Composition Fields; Correct the composition inputs and rerun mise run generate")
        libraries = [(composition.parent / library["descriptors"], library["packages"]) for library in settings.get("libraries", [])]
    else:
        libraries = [(repository / "crates/harmos-protocol/src/descriptors.bin", [])]
    for path, packages in [*libraries, *((path, []) for path in descriptors)]:
        library = FileDescriptorSet.FromString(path.read_bytes())
        existing_packages = {file.package for file in library.file}
        if set(packages) - existing_packages:
            raise ValueError(
                "Compose SDK: Library Package Is Absent from Its Descriptors; Correct the composition inputs and rerun mise run generate"
            )
        for file in library.file:
            if (
                packages
                and not file.package.startswith("google.")
                and not any(file.package == package or file.package.startswith(package + ".") for package in packages)
            ):
                continue
            file.ClearField("source_code_info")
            if file.name in files and files[file.name] != file:
                raise ValueError(
                    f"Generate Client: Conflicting Descriptor File {file.name}; Correct the composition inputs and rerun mise run generate"
                )
            files[file.name] = file
    merged.file.extend(files[name] for name in sorted(files))
    if settings.get("callers"):
        narrow_callers(merged, settings["callers"])
    methods = harmos_tools.build.resources.descriptor.table(merged)
    clients = [method for method in methods if method["face"] == "client"]
    contexts = [method for method in methods if method["face"] == "context"]
    if language == "python":
        packages = {file.package.split(".", 1)[0] for file in merged.file if not file.package.startswith(("harmos.", "google."))}
        harmos_tools.build.resources.python.client(
            output,
            clients,
            merged,
            module_prefixes={package: f"{namespace}._wire.{package}" for package in packages},
        )
        plugin = output.with_name("plugin.py")
        harmos_tools.build.resources.python.host(
            plugin,
            contexts,
            merged,
            module_prefixes={package: f"{namespace}._wire.{package}" for package in packages},
            context_wrapper=True,
        )
    elif language == "csharp":
        harmos_tools.build.resources.csharp.client(repository, clients, namespace=namespace, target=output, check=check)
        harmos_tools.build.resources.csharp.host(
            repository,
            contexts,
            target=output.with_name("Plugin.g.cs"),
            namespace=".".join(part.title() for part in namespace.split(".")) + ".Plugins",
            check=check,
        )
    elif language == "rust":
        harmos_tools.build.resources.rust.client(repository, clients, target=output)
        harmos_tools.build.resources.rust.host(repository, contexts, target=output.with_name("plugin.rs"))
    elif language == "cpp":
        harmos_tools.build.resources.cpp.client(repository, clients, namespace=namespace, target=output)
        harmos_tools.build.resources.cpp.host(repository, contexts, target=output.with_name("plugin.hpp"), namespace=namespace)
    else:
        generate(output, merged, namespace=namespace)


def main() -> None:
    parser = ArgumentParser(description=__doc__)
    parser.add_argument("--descriptors", type=Path, action="append", default=[])
    parser.add_argument("--output", type=Path)
    parser.add_argument("--namespace", default="harmos")
    parser.add_argument("--composition", type=Path)
    parser.add_argument("--language", choices=("python",))
    parser.add_argument("--cpp-fixture", type=Path)
    options = parser.parse_args()
    root = Path(__file__).resolve().parents[5]
    if options.cpp_fixture:
        fixture = root / "sdk/rust/tests/fixtures/client/client-descriptors.bin"
        library = root / "crates/harmos-protocol/src/descriptors.bin"
        methods = harmos_tools.build.resources.descriptor.table(
            FileDescriptorSet.FromString(library.read_bytes())
        ) + harmos_tools.build.resources.descriptor.table(FileDescriptorSet.FromString(fixture.read_bytes()))
        harmos_tools.build.resources.cpp.client(
            root, [method for method in methods if method["face"] == "client"], namespace="fixture", target=options.cpp_fixture
        )
    elif options.descriptors:
        if options.output is None:
            parser.error("composed client descriptors require --output")
        compose(root, options.descriptors, options.output, options.namespace, language=options.language, composition=options.composition)
    else:
        generate(root)


# === Private Functions === #

# === Entry Point === #

if __name__ == "__main__":
    main()
