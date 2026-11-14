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
import argparse
import filecmp
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

# Third Party Imports
import grpc_tools
from google.protobuf.descriptor_pb2 import FileDescriptorSet

# Local Imports
from ..process import spawn_flags
from .resource_roots import python, table

# Type Checking Imports

# === Public Exports === #

# === Constants === #

ROOT = Path(__file__).resolve().parents[5]
PROTOCOLS = tuple(
    sorted(
        path.relative_to(ROOT / "proto").as_posix()
        for face in ("v1", "client/v1", "plugin")
        for path in (ROOT / "proto/harmos" / face).rglob("*.proto")
        if path.name != "guest.proto"
    )
)
GENERATED = tuple(
    name.removesuffix(".proto").replace("harmos/client/", "harmos/_wire/client/", 1) + suffix
    for name in PROTOCOLS
    for suffix in ("_pb2.py", "_pb2.pyi", "_pb2_grpc.py")
)

# === Types === #

# === Classes === #

# === Public Functions === #


def generate(output: Path) -> None:
    output.mkdir(parents=True, exist_ok=True)
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
    # Keep Generated Client Modules Separate From the Authored harmos.client Module.
    client = output / "harmos/client"
    destination = output / "harmos/_wire/client"
    if client.is_dir():
        destination.parent.mkdir(parents=True, exist_ok=True)
        shutil.copytree(client, destination, dirs_exist_ok=True)
        shutil.rmtree(client)
    for name in GENERATED:
        path = output / name
        source = path.read_text(encoding="utf-8")
        source = source.replace("from harmos.client", "from harmos._wire.client")
        source = source.replace("import harmos.client.", "import harmos._wire.client.")
        source = source.replace("'harmos.client.v1.", "'harmos._wire.client.v1.")
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
    python(output / "harmos/generated/client.py", [method for method in table(descriptors) if method["face"] == "client"], descriptors)


def generate_client_fixture(output: Path) -> None:
    proto = ROOT / "sdk/tests/fixtures/client/proto"
    inputs = sorted(path.relative_to(proto).as_posix() for path in proto.rglob("*.proto"))
    output.mkdir(parents=True, exist_ok=True)
    scratch = Path(tempfile.mkdtemp())
    try:
        descriptor_path = scratch / "client-descriptors.bin"
        subprocess.run(
            [
                sys.executable,
                "-m",
                "grpc_tools.protoc",
                f"-I{proto}",
                f"--python_out={output}",
                f"--pyi_out={output}",
                f"--grpc_python_out={output}",
                f"--descriptor_set_out={descriptor_path}",
                "--include_imports",
                *inputs,
            ],
            check=True,
            cwd=ROOT,
            creationflags=spawn_flags(),
        )
        descriptors = FileDescriptorSet.FromString((ROOT / "crates/harmos-protocol/src/descriptors.bin").read_bytes())
        descriptors.file.extend(FileDescriptorSet.FromString(descriptor_path.read_bytes()).file)
        python(
            output / "client.py",
            [method for method in table(descriptors) if method["face"] == "client"],
            descriptors,
            module_prefixes={"application": "tests.fixtures.client.application"},
        )
    finally:
        shutil.rmtree(scratch)


def generate_conformance(output: Path) -> None:
    proto = ROOT / "sdk/tests/fixtures/conformance/proto"
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
                "harmos/conformance/v1/fixture.proto",
            ],
            check=True,
            cwd=ROOT,
            creationflags=spawn_flags(),
        )
        output.mkdir(parents=True, exist_ok=True)
        for source in (Path(scratch) / "harmos/conformance/v1").iterdir():
            shutil.copyfile(source, output / source.name)
    finally:
        shutil.rmtree(scratch)


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true")
    options = parser.parse_args()
    committed = ROOT / "sdk/python/src"
    if not options.check:
        generate(committed)
        generate_client_fixture(ROOT / "sdk/python/tests/fixtures/client")
        generate_conformance(ROOT / "sdk/python/tests/fixtures/conformance/v1")
        return

    scratch = tempfile.mkdtemp()
    try:
        output = Path(scratch)
        generate(output)
        changed = [
            name
            for name in (*GENERATED, "harmos/generated/client.py")
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
        fixture = ROOT / "sdk/python/tests/fixtures/conformance/v1"
        generated_fixture = output / "conformance"
        generate_conformance(generated_fixture)
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
