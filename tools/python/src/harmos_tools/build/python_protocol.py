# ***
#
# Harmos
#
# SPDX-License-Identifier: Apache-2.0
#
# ***

"""Generate and Compare the Python Protocol Bindings."""

# === Imports === #

# Standard Library Imports
import filecmp
import shutil
import subprocess
import sys
import tempfile
from argparse import ArgumentParser
from dataclasses import replace
from pathlib import Path

# Third Party Imports
import grpc_tools
from google.protobuf.descriptor_pb2 import FileDescriptorSet

# Local Imports
import harmos_tools.build.resource_roots
import harmos_tools.build.resources.descriptor
import harmos_tools.build.resources.python
from harmos_tools.build.resources.configuration import Configuration

from ..process import spawn_flags

# Type Checking Imports

# === Public Exports === #

# === Constants === #

ROOT = Path(__file__).resolve().parents[5]
# === Types === #

# === Classes === #

# === Public Functions === #


def protocols(*, root: Path = ROOT, namespace: str = "harmos", proto_root: Path | None = None) -> tuple[str, ...]:
    proto_root = proto_root or root / "proto"
    return tuple(sorted(path.relative_to(proto_root).as_posix() for path in (proto_root / namespace).rglob("*.proto")))


def generate(
    output: Path,
    *,
    root: Path = ROOT,
    namespace: str = "harmos",
    proto_root: Path | None = None,
    descriptors: Path | None = None,
    files: tuple[str, ...] | None = None,
    module_prefixes: dict[str, str] | None = None,
    configuration: Configuration | None = None,
) -> None:
    """Compile Product-Owned Wire Modules and Render Their Typed Resources."""
    configuration = configuration or Configuration(namespace=namespace)
    namespace = configuration.namespace
    python_package = configuration.python_package if configuration.python_package is not None else namespace
    proto_root = proto_root or root / "proto"
    definitions = files if files is not None else protocols(root=root, namespace=namespace, proto_root=proto_root)
    if not definitions:
        raise ValueError("Generate Python Protocol: Supply Protocol Files or a Populated proto_root/namespace Directory")
    generated = tuple(
        name.removesuffix(".proto").replace(namespace + "/", python_package + "/_wire/", 1) + suffix
        for name in definitions
        for suffix in ("_pb2.py", "_pb2.pyi", "_pb2_grpc.py")
    )
    output.mkdir(parents=True, exist_ok=True)
    for path in (output / python_package).rglob("*_pb2*"):
        name = path.relative_to(output).as_posix()
        if path.is_file() and path.name.endswith(("_pb2.py", "_pb2.pyi", "_pb2_grpc.py")) and name not in generated:
            path.unlink()
    command = [
        sys.executable,
        "-m",
        "grpc_tools.protoc",
        f"-I{proto_root}",
        f"-I{Path(grpc_tools.__file__).parent / '_proto'}",
        f"--python_out={output}",
        f"--pyi_out={output}",
        f"--grpc_python_out={output}",
        *definitions,
    ]
    subprocess.run(command, check=True, cwd=root, creationflags=spawn_flags())
    prefixes = {"harmos": "harmos._wire", namespace: python_package + "._wire", **(module_prefixes or {})}
    for name in definitions:
        for suffix in ("_pb2.py", "_pb2.pyi", "_pb2_grpc.py"):
            relative = name.removesuffix(".proto") + suffix
            source_path = output / relative
            destination = output / relative.replace(namespace + "/", python_package + "/_wire/", 1)
            source = source_path.read_text(encoding="utf-8")
            for prefix, replacement in sorted(prefixes.items(), key=lambda item: -len(item[0])):
                source = source.replace(f"from {prefix}.", f"from {replacement}.")
                source = source.replace(f"import {prefix}.", f"import {replacement}.")
                source = source.replace(f"from {prefix} import ", f"from {replacement} import ")
                source = source.replace(
                    f"BuildTopDescriptorsAndMessages(DESCRIPTOR, '{prefix}.", f"BuildTopDescriptorsAndMessages(DESCRIPTOR, '{replacement}."
                )
            if suffix == "_pb2_grpc.py" and "grpc.experimental." in source:
                source = source.replace("import grpc\n", "import grpc\nimport grpc.experimental\n", 1)
            destination.parent.mkdir(parents=True, exist_ok=True)
            destination.write_text(source, encoding="utf-8", newline="\n")
            if source_path != destination:
                source_path.unlink()
    descriptor_path = descriptors or root / "crates/harmos-protocol/src/descriptors.bin"
    schema = FileDescriptorSet.FromString(descriptor_path.read_bytes())
    if configuration.library is None:
        configuration = replace(configuration, library=schema)
    methods = harmos_tools.build.resources.descriptor.table(schema)
    harmos_tools.build.resources.python.exports(output / python_package / "_generated", schema, configuration=configuration)
    harmos_tools.build.resources.python.host(
        output / python_package / "_generated/host.py",
        [method for method in methods if method["face"] == "context"],
        schema,
        configuration=configuration,
        module_prefixes=module_prefixes,
    )
    harmos_tools.build.resources.python.client(
        output / python_package / "_generated/client.py",
        [method for method in methods if method["face"] == "client"],
        schema,
        configuration=configuration,
        module_prefixes=module_prefixes,
    )


def generate_client_fixture(output: Path) -> None:
    proto = ROOT / "sdk/tests/fixtures/client/proto"
    inputs = sorted(path.relative_to(proto).as_posix() for path in proto.rglob("*.proto"))
    output.mkdir(parents=True, exist_ok=True)
    wire = output / "_wire"
    wire.mkdir(parents=True, exist_ok=True)
    generated = {name.removesuffix(".proto") + suffix for name in inputs for suffix in ("_pb2.py", "_pb2.pyi", "_pb2_grpc.py")}
    for path in wire.rglob("*_pb2*"):
        if path.is_file() and path.name.endswith((".py", ".pyi")) and path.relative_to(wire).as_posix() not in generated:
            path.unlink()
    scratch = Path(tempfile.mkdtemp())
    try:
        descriptor_path = scratch / "client-descriptors.bin"
        # Run the Native Command
        command = [
            sys.executable,
            "-m",
            "grpc_tools.protoc",
            f"-I{proto}",
            f"-I{ROOT / 'proto'}",
            f"-I{Path(grpc_tools.__file__).parent / '_proto'}",
            f"--python_out={wire}",
            f"--pyi_out={wire}",
            f"--grpc_python_out={wire}",
            f"--descriptor_set_out={descriptor_path}",
            *inputs,
        ]
        subprocess.run(
            command,
            check=True,
            cwd=ROOT,
            creationflags=spawn_flags(),
        )
        for path in wire.rglob("*_pb2*"):
            if not path.is_file() or not path.name.endswith((".py", ".pyi")):
                continue
            source = path.read_text(encoding="utf-8").replace("from application.", "from tests.fixtures.client._wire.application.")
            source = source.replace(
                "BuildTopDescriptorsAndMessages(DESCRIPTOR, 'application.",
                "BuildTopDescriptorsAndMessages(DESCRIPTOR, 'tests.fixtures.client._wire.application.",
            )
            source = source.replace("from harmos.model.", "from harmos._wire.model.")
            path.write_text(source, encoding="utf-8", newline="\n")
        harmos_tools.build.resource_roots.compose(
            ROOT,
            [descriptor_path],
            output / "_generated/client.py",
            "tests.fixtures.client",
            language="python",
        )
    finally:
        shutil.rmtree(scratch)


def generate_conformance(output: Path) -> None:
    proto = ROOT / "sdk/tests/fixtures/conformance/proto"
    inputs = ["conformance/model/v1/fixture.proto"]
    scratch = tempfile.mkdtemp()
    try:
        # Run the Native Command
        command = [
            sys.executable,
            "-m",
            "grpc_tools.protoc",
            f"-I{proto}",
            f"-I{Path(grpc_tools.__file__).parent / '_proto'}",
            f"--python_out={scratch}",
            f"--pyi_out={scratch}",
            f"--grpc_python_out={scratch}",
            f"--descriptor_set_out={Path(scratch) / 'descriptors.bin'}",
            "--include_imports",
            *inputs,
        ]
        subprocess.run(
            command,
            check=True,
            cwd=ROOT,
            creationflags=spawn_flags(),
        )
        output.mkdir(parents=True, exist_ok=True)
        for name in inputs:
            for suffix in ("_pb2.py", "_pb2.pyi", "_pb2_grpc.py"):
                source = Path(scratch) / (name.removesuffix(".proto") + suffix)
                shutil.copyfile(source, output / source.name)
        schema = FileDescriptorSet.FromString((Path(scratch) / "descriptors.bin").read_bytes())
        names = sorted(message.name for file in schema.file if file.name in inputs for message in file.message_type)
        package = output.parents[2]
        types = [
            "# @generated from protocol descriptors; do not edit.",
            *[f"from ._wire.model.v1.fixture_pb2 import {name}" for name in names],
            "",
            f"__all__ = {names!r}",
        ]
        target = package / "_types.py"
        target.write_text("\n".join(types) + "\n", encoding="utf-8", newline="\n")
        config = ROOT / "sdk/python/pyproject.toml"
        subprocess.run(
            [sys.executable, "-m", "ruff", "check", "--config", str(config), "--select", "I,F401", "--fix", str(target)],
            check=True,
            creationflags=spawn_flags(),
        )
        subprocess.run(
            [sys.executable, "-m", "ruff", "format", "--config", str(config), str(target)], check=True, creationflags=spawn_flags()
        )
    finally:
        shutil.rmtree(scratch)


def main() -> None:
    parser = ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true")
    options = parser.parse_args()
    definitions = protocols()
    generated = tuple(
        name.removesuffix(".proto").replace("harmos/", "harmos/_wire/", 1) + suffix
        for name in definitions
        for suffix in ("_pb2.py", "_pb2.pyi", "_pb2_grpc.py")
    )
    committed = ROOT / "sdk/python/src"
    if not options.check:
        generate(committed)
        generate_client_fixture(ROOT / "sdk/python/tests/fixtures/client")
        generate_conformance(ROOT / "sdk/python/tests/fixtures/conformance/_wire/model/v1")
        return

    scratch = tempfile.mkdtemp()
    try:
        output = Path(scratch)
        generate(output)
        expected = set(generated) | {"harmos/__init__.py", "harmos/client.py", "harmos/plugin/__init__.py"}
        expected.update(path.relative_to(output).as_posix() for path in (output / "harmos/_generated").rglob("*.py"))
        existing = set()
        for path in (committed / "harmos").rglob("*"):
            if path.is_file() and (path.name.endswith(("_pb2.py", "_pb2.pyi", "_pb2_grpc.py")) or "_generated" in path.parts):
                existing.add(path.relative_to(committed).as_posix())
        changed = [
            name
            for name in sorted(expected)
            if not (committed / name).is_file() or not filecmp.cmp(output / name, committed / name, shallow=False)
        ]
        changed += sorted(existing - expected)
        generated_client = output / "client"
        generate_client_fixture(generated_client)
        client = ROOT / "sdk/python/tests/fixtures/client"
        changed += [
            str(path.relative_to(output))
            for path in generated_client.rglob("*")
            if path.is_file()
            and (
                not (client / path.relative_to(generated_client)).exists()
                or not filecmp.cmp(path, client / path.relative_to(generated_client), shallow=False)
            )
        ]
        fixture = ROOT / "sdk/python/tests/fixtures/conformance/_wire/model/v1"
        generated_fixture = output / "conformance/_wire/model/v1"
        generate_conformance(generated_fixture)
        types = generated_fixture.parents[2] / "_types.py"
        committed_types = ROOT / "sdk/python/tests/fixtures/conformance/_types.py"
        if not committed_types.is_file() or types.read_bytes() != committed_types.read_bytes():
            changed.append("conformance/_types.py")
        changed += [
            str(path.relative_to(output))
            for path in generated_fixture.iterdir()
            if not (fixture / path.name).exists() or not filecmp.cmp(path, fixture / path.name, shallow=False)
        ]
        if changed:
            raise SystemExit(f"Generated Python Differs: {', '.join(changed)}; Run mise run generate -- --sdk python")
    finally:
        shutil.rmtree(scratch)


# === Private Functions === #

# === Entry Point === #

if __name__ == "__main__":
    main()
