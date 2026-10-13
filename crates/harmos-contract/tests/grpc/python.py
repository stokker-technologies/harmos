# ***
#
# Harmos
#
# SPDX-License-Identifier: Apache-2.0
#
# ***

"""Generated Package And Protobuf Consumer Boundary Fixture"""

# === Imports === #

# Standard Library Imports
import hashlib
import importlib
import json
import os
import shutil
import subprocess
import sys
import tarfile
import tomllib
import zipfile
from pathlib import Path
from typing import Any

# Third Party Imports
from google.protobuf import descriptor_pool, json_format
from google.protobuf.descriptor import FieldDescriptor, Descriptor
from google.protobuf.message import Message

# Local Imports

# Type Checking Imports

# === Public Exports === #

# === Constants === #

# === Types === #

# === Classes === #

# === Public Functions === #


def main() -> None:
    target, alpha, beta = map(Path, sys.argv[1:])
    sys.path[:0] = [str(alpha / "src"), str(beta / "src")]
    first = importlib.import_module("bindings_alpha")
    second = importlib.import_module("bindings_beta")
    operations = [
        value for value in vars(first).values() if isinstance(value, first.Operation)
    ]
    peers = {
        value.id: value
        for value in vars(second).values()
        if isinstance(value, second.Operation)
    }
    assert len(operations) == 7
    assert {value.mode for value in operations} == {
        "unary",
        "client_stream",
        "server_stream",
        "bidi",
    }
    for operation in operations:
        assert issubclass(operation.request, Message)
        assert operation.request is operation.response
        assert operation.codec == "protobuf"
        assert (
            hashlib.sha256(operation.contract.encode()).hexdigest()
            == operation.fingerprint
        )
        assert (
            operation.request.DESCRIPTOR.full_name
            == peers[operation.id].request.DESCRIPTOR.full_name
        )
        assert operation.request is not peers[operation.id].request
        assert (
            operation.request.DESCRIPTOR.file.pool
            is not peers[operation.id].request.DESCRIPTOR.file.pool
        )
        try:
            descriptor_pool.Default().FindMessageTypeByName(
                operation.request.DESCRIPTOR.full_name
            )
        except KeyError:
            pass
        else:
            raise AssertionError("producer leaked into the default protobuf pool")
        try:
            operation.request().MergeFrom(peers[operation.id].request())
        except (TypeError, ValueError):
            pass
        else:
            raise AssertionError("cross-producer class identity was accepted")
        if operation.input is not None:
            assert operation.input is operation.request
            assert operation.rpc_request.DESCRIPTOR.oneofs_by_name["value"]
        if operation.output is not None:
            assert operation.output is operation.response
            assert operation.rpc_response.DESCRIPTOR.oneofs_by_name["value"]
        if operation.id == "fixture.checkpoint":
            schema = json.loads(operation.contract)["schema"]
            assert len(schema["definitions"]) == 165
            assert sum(map(len, schema["definitions"])) == 609
            assert operation.version == 2
            graph(operation.request.DESCRIPTOR.file.pool, schema)
        elif operation.id == "fixture.reserved":
            contract = json.loads(operation.contract)
            raw = contract["schema"]["definitions"][0][0]
            assert raw["name"] == "Clear" and raw["tag"] == 7
            dynamic = importlib.import_module("bindings_alpha.dynamic").from_contract(
                contract
            )
            for projected in (operation, dynamic):
                field = projected.request.DESCRIPTOR.fields_by_number[7]
                assert field.name == "Clear_" and field.json_name == "Clear"
                value = projected.request()
                value.Clear_.SetInParent()
                assert value.SerializeToString() == b"\x3a\x00"
                assert json_format.MessageToDict(value) == {"Clear": {}}
                copy = projected.request()
                json_format.ParseDict({"Clear": {}}, copy)
                assert copy.SerializeToString() == value.SerializeToString()
                assert callable(copy.Clear)
                copy.Clear()
                assert copy.SerializeToString() == b""
        elif operation.id == "fixture.recursive":
            descriptor = operation.request.DESCRIPTOR
            assert descriptor.fields_by_name["next"].message_type is descriptor
            value = operation.request()
            value.next.next.SetInParent()
            copy = operation.request.FromString(value.SerializeToString())
            assert copy.next.HasField("next")
        else:
            value = operation.request.FromString((target / "rust.bin").read_bytes())
            assert value.text == "line\n∞"
            assert value.HasField("optional") and value.optional == 2**32 - 1
            assert list(value.values) == [-(2**63), -1, 0, 2**63 - 1]
            (target / "python.bin").write_bytes(value.SerializeToString())
    typing(target, alpha / "src", beta / "src")
    metadata = tomllib.loads((alpha / "pyproject.toml").read_text())
    assert metadata["project"]["version"] == "1.2.3rc4"
    assert metadata["project"]["dependencies"] == ["protobuf>=7.36.2"]
    assert (alpha / "src/bindings_alpha/py.typed").is_file()
    if os.environ.get("HARMOS_BINDINGS_SKIP_INSTALL"):
        return
    install(target, alpha, beta)


# === Private Functions === #


def graph(pool: descriptor_pool.DescriptorPool, schema: dict[str, Any]) -> None:

    namespace = "test.v1.ShapesMessages.Checkpoint"

    def row(index: int) -> Descriptor:
        root = schema["root"]
        number = 0 if index == root else index + 1 if index < root else index
        return pool.FindMessageTypeByName(f"{namespace}.Row{number}")

    for index, fields in enumerate(schema["definitions"]):
        descriptor = row(index)
        assert len(descriptor.fields) == len(fields)
        for field in fields:
            actual = descriptor.fields_by_number[field["tag"]]
            assert actual.name == field["name"] or actual.json_name == field["name"]
            assert actual.number == field["tag"]
            assert actual.is_repeated == (field["cardinality"] == "repeated")
            if field["cardinality"] in ("optional", "variant"):
                assert actual.has_presence
            if field["cardinality"] == "variant":
                assert actual.containing_oneof is not None
            if field["kind"]["type"] == "message":
                assert actual.type == FieldDescriptor.TYPE_MESSAGE
                assert actual.message_type is row(field["kind"]["definition"])


def typing(target: Path, alpha: Path, beta: Path) -> None:
    checker = Path(sys.executable).parent / ("ty.exe" if os.name == "nt" else "ty")
    env = dict(os.environ, PYTHONPATH=os.pathsep.join([str(alpha), str(beta)]))
    source = target / "consumer.py"
    source.write_text(
        "from bindings_alpha import FIXTURE_RECURSIVE, FixtureRecursiveRequest\n"
        "def next(value: FixtureRecursiveRequest) -> FixtureRecursiveRequest:\n"
        "    return FIXTURE_RECURSIVE.request(next=value)\n"
        "from bindings_alpha import FixtureReservedRequest\n"
        "reserved = FixtureReservedRequest()\n"
        "reserved.Clear_.SetInParent()\n"
        "reserved.Clear()\n"
    )
    command = [
        str(checker),
        "check",
        "--python-version",
        "3.13",
        "--python",
        sys.executable,
        "--extra-search-path",
        str(alpha),
        "--extra-search-path",
        str(beta),
        str(source),
    ]
    result = subprocess.run(command, env=env, capture_output=True, text=True)
    assert result.returncode == 0, result.stdout + result.stderr
    source.write_text(
        "from bindings_alpha import FIXTURE_RECURSIVE\n"
        "FIXTURE_RECURSIVE.request(next='invalid')\n"
        "from bindings_alpha import FixtureReservedRequest\n"
        "FixtureReservedRequest(Clear_=123)\n"
    )
    result = subprocess.run(command, env=env, capture_output=True, text=True)
    assert result.returncode != 0 and "invalid-argument-type" in result.stdout, (
        result.stdout + result.stderr
    )


def _run(command: list[str], *, env: dict[str, str], cwd: Path | None = None) -> None:
    result = subprocess.run(command, env=env, cwd=cwd, capture_output=True, text=True)
    assert result.returncode == 0, (
        f"Command: {command}\n{result.stdout}\n{result.stderr}"
    )


def install(target: Path, alpha: Path, beta: Path) -> None:
    uv = shutil.which("uv")
    assert uv is not None
    env = dict(os.environ, PATH=os.defpath)
    dist = target / "distributions"
    _run(
        [
            sys.executable,
            "-m",
            "build",
            "--no-isolation",
            "--outdir",
            str(dist),
            str(alpha),
        ],
        env=env,
    )
    wheel = next(dist.glob("*.whl"))
    sdist = next(dist.glob("*.tar.gz"))
    with zipfile.ZipFile(wheel) as archive:
        names = archive.namelist()
        assert "bindings_alpha/py.typed" in names
        assert "bindings_alpha/__init__.pyi" in names
        assert "bindings_alpha/contracts.json" in names
        assert "bindings_alpha/descriptors.bin" in names
        assert any(name.endswith("_pb2.pyi") for name in names)
        assert not any(name.endswith((".so", ".pyd", ".dll")) for name in names)
    with tarfile.open(sdist) as archive:
        assert not any(
            name.endswith(("Cargo.toml", ".rs")) for name in archive.getnames()
        )
    # Remove Producer Source From Both Installations' Import And Build Paths
    alpha.rename(target / "producer-source-held")
    for archive in (wheel, sdist):
        destination = target / (
            "wheel-installed" if archive == wheel else "sdist-installed"
        )
        _run(
            [
                uv,
                "pip",
                "install",
                "--offline",
                "--no-deps",
                "--no-build-isolation",
                "--python",
                sys.executable,
                "--target",
                str(destination),
                str(archive),
            ],
            env=env,
        )
        code = """
import importlib, json, sys
from pathlib import Path
sys.path[:0] = [sys.argv[1], sys.argv[2]]
pkg = importlib.import_module('bindings_alpha')
assert Path(pkg.__file__).is_relative_to(Path(sys.argv[1]))
ops = [value for value in vars(pkg).values() if isinstance(value, pkg.Operation)]
assert len(ops) == 7
for operation in ops:
    assert operation.request is operation.response
    assert json.loads(operation.contract)['version'] == operation.version
    operation.request.FromString(operation.request().SerializeToString())
import bindings_beta
assert pkg.FIXTURE_RECURSIVE.request is not bindings_beta.FIXTURE_RECURSIVE.request
"""
        _run(
            [sys.executable, "-c", code, str(destination), str(beta / "src")],
            env=env,
            cwd=target,
        )
        typing(target, destination, beta / "src")
    (target / "producer-source-held").rename(alpha)


# === Entry Point === #

if __name__ == "__main__":
    main()
