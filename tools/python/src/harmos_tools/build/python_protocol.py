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
from pathlib import Path

# Third Party Imports
import grpc_tools
from google.protobuf.descriptor_pb2 import FileDescriptorSet

# Local Imports
from ..process import spawn_flags
from .resource_roots import compose, python, python_exports, python_host, table

# Type Checking Imports

# === Public Exports === #

# === Constants === #

ROOT = Path(__file__).resolve().parents[5]
PROTOCOLS = tuple(
    sorted(
        path.relative_to(ROOT / "proto").as_posix()
        for face in ("model", "host", "plugin")
        for path in (ROOT / "proto/harmos" / face).rglob("*.proto")
    )
)
GENERATED = tuple(
    name.removesuffix(".proto").replace("harmos/", "harmos/_wire/", 1) + suffix
    for name in PROTOCOLS
    for suffix in ("_pb2.py", "_pb2.pyi", "_pb2_grpc.py")
)

# === Types === #

# === Classes === #

# === Public Functions === #


def generate(output: Path) -> None:
    output.mkdir(parents=True, exist_ok=True)
    for name in ["harmos/v1/__init__.py", "harmos/plugin/v1/__init__.py"]:
        (output / name).unlink(missing_ok=True)
    for path in (output / "harmos").rglob("*_pb2*"):
        name = path.relative_to(output).as_posix()
        if path.is_file() and path.name.endswith(("_pb2.py", "_pb2.pyi", "_pb2_grpc.py")) and name not in GENERATED:
            path.unlink()
    command = [
        sys.executable,
        "-m",
        "grpc_tools.protoc",
        f"-I{ROOT / 'proto'}",
        f"-I{Path(grpc_tools.__file__).parent / '_proto'}",
        f"--python_out={output}",
        f"--pyi_out={output}",
        f"--grpc_python_out={output}",
        *PROTOCOLS,
    ]
    subprocess.run(command, check=True, cwd=ROOT, creationflags=spawn_flags())
    # All Protocol Packages Are Private Implementation Modules.
    for face in ("model", "host", "plugin"):
        source = output / "harmos" / face
        destination = output / "harmos/_wire" / face
        if source.is_dir():
            for path in source.rglob("*_pb2*"):
                if not path.is_file() or not path.name.endswith(("_pb2.py", "_pb2.pyi", "_pb2_grpc.py")):
                    continue
                target = destination / path.relative_to(source)
                target.parent.mkdir(parents=True, exist_ok=True)
                shutil.move(str(path), target)
    for name in GENERATED:
        path = output / name
        source = path.read_text(encoding="utf-8")
        for face in ("model", "host", "plugin"):
            source = source.replace(f"from harmos.{face}", f"from harmos._wire.{face}")
            source = source.replace(f"import harmos.{face}.", f"import harmos._wire.{face}.")
            source = source.replace(f"'harmos.{face}.", f"'harmos._wire.{face}.")
        path.write_text(source, encoding="utf-8", newline="\n")
    # Generated Convenience Calls Require the Experimental Submodule Explicitly.
    for name in GENERATED:
        if name.endswith("_pb2_grpc.py"):
            path = output / name
            source = path.read_text(encoding="utf-8")
            if "grpc.experimental." in source:
                path.write_text(
                    source.replace("import grpc\n", "import grpc\nimport grpc.experimental\n", 1), encoding="utf-8", newline="\n"
                )
    descriptors = FileDescriptorSet.FromString((ROOT / "crates/harmos-protocol/src/descriptors.bin").read_bytes())
    python_exports(output / "harmos/_generated", descriptors)
    python_host(output / "harmos/_generated/host.py", [method for method in table(descriptors) if method["face"] == "context"], descriptors)
    python(output / "harmos/_generated/client.py", [method for method in table(descriptors) if method["face"] == "client"], descriptors)


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
    for path in (output / "application").rglob("*_pb2*"):
        if path.is_file() and path.name.endswith((".py", ".pyi")):
            path.unlink()
    scratch = Path(tempfile.mkdtemp())
    try:
        descriptor_path = scratch / "client-descriptors.bin"
        subprocess.run(
            [
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
            ],
            check=True,
            cwd=ROOT,
            creationflags=spawn_flags(),
        )
        for path in wire.rglob("*_pb2*"):
            if not path.is_file() or not path.name.endswith((".py", ".pyi")):
                continue
            source = path.read_text(encoding="utf-8").replace("from application.", "from tests.fixtures.client._wire.application.")
            source = source.replace("'application.", "'tests.fixtures.client._wire.application.")
            source = source.replace("from harmos.model.", "from harmos._wire.model.")
            path.write_text(source, encoding="utf-8", newline="\n")
        compose(
            ROOT,
            [descriptor_path],
            output / "_generated/client.py",
            "tests.fixtures.client",
            language="python",
            composition=proto.parent / "composition.toml",
        )
    finally:
        shutil.rmtree(scratch)


def generate_conformance(output: Path) -> None:
    proto = ROOT / "sdk/tests/fixtures/conformance/proto"
    inputs = sorted(path.relative_to(proto).as_posix() for path in proto.rglob("*.proto"))
    scratch = tempfile.mkdtemp()
    try:
        subprocess.run(
            [
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
            ],
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
        for folder in (package / "v1", package / "_wire/v1"):
            for obsolete in folder.glob("*_pb2*"):
                if obsolete.is_file():
                    obsolete.unlink()
        types = [
            "# @generated from protocol descriptors; do not edit.",
            *[f"from ._wire.model.v1.fixture_pb2 import {name} as {name}" for name in names],
        ]
        (package / "_types.py").write_text("\n".join(types) + "\n", encoding="utf-8", newline="\n")
    finally:
        shutil.rmtree(scratch)


def main() -> None:
    parser = ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true")
    options = parser.parse_args()
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
        changed = [
            name
            for name in (
                *GENERATED,
                "harmos/_generated/client.py",
                "harmos/_generated/shared.py",
                "harmos/_generated/client_types.py",
                "harmos/_generated/plugin_types.py",
                "harmos/_generated/host.py",
                "harmos/__init__.py",
                "harmos/client.py",
                "harmos/plugin/__init__.py",
            )
            if not (committed / name).exists() or not filecmp.cmp(output / name, committed / name, shallow=False)
        ]
        changed += [
            path.relative_to(committed).as_posix()
            for path in (committed / "harmos").rglob("*_pb2*")
            if path.is_file()
            and path.name.endswith(("_pb2.py", "_pb2.pyi", "_pb2_grpc.py"))
            and path.relative_to(committed).as_posix() not in GENERATED
        ]
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
