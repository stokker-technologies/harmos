# ***
#
# Harmos
#
# SPDX-License-Identifier: Apache-2.0
#
# ***

"""Consume Generated Python Packages, One Scene Per Acceptance Story"""

# === Imports === #

# Standard Library Imports
import copy
import hashlib
import importlib
import json
import os
import shutil
import subprocess
import sys
import tarfile
import tomllib
import types
import zipfile
from collections.abc import Callable
from pathlib import Path
from typing import Any

# Third Party Imports
from google.protobuf import descriptor_pool, json_format
from google.protobuf.descriptor import Descriptor, FieldDescriptor
from google.protobuf.message import Message

# Local Imports

# Type Checking Imports

# === Public Exports === #

# === Constants === #

# === Types === #

# === Classes === #

# === Public Functions === #


def wire(target: Path, alpha: Path, beta: Path) -> None:
    """Decode The Rust Payload From Stdin And Echo Its Python Encoding."""
    package = _imported(alpha, beta)[0]
    encoded = sys.stdin.buffer.read()
    value = None
    for operation in _operations(package):
        if operation.id.startswith("fixture."):
            continue
        value = operation.request.FromString(encoded)
        assert value.text == "line\n∞"
        assert value.HasField("optional") and value.optional == 2**32 - 1
        assert list(value.values) == [-(2**63), -1, 0, 2**63 - 1]
    assert value is not None
    sys.stdout.buffer.write(value.SerializeToString())


def descriptors(target: Path, alpha: Path, beta: Path) -> None:
    """Generated Operations Carry Their Contract's Modes, Codec And Schema Graph."""
    package = _imported(alpha, beta)[0]
    operations = _operations(package)
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
        digest = hashlib.sha256(operation.contract.encode()).hexdigest()
        assert digest == operation.fingerprint
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
            _graph(operation.request.DESCRIPTOR.file.pool, schema)
        elif operation.id == "fixture.reserved":
            _reserved(package, operation)
        elif operation.id == "fixture.recursive":
            descriptor = operation.request.DESCRIPTOR
            assert descriptor.fields_by_name["next"].message_type is descriptor
            value = operation.request()
            value.next.next.SetInParent()
            copy_value = operation.request.FromString(value.SerializeToString())
            assert copy_value.next.HasField("next")


def isolation(target: Path, alpha: Path, beta: Path) -> None:
    """Each Producer Owns A Private Descriptor Pool; Classes Never Cross."""
    first, second = _imported(alpha, beta)
    peers = {value.id: value for value in _operations(second)}
    for operation in _operations(first):
        peer = peers[operation.id]
        name = operation.request.DESCRIPTOR.full_name
        assert name == peer.request.DESCRIPTOR.full_name
        assert operation.request is not peer.request
        assert (
            operation.request.DESCRIPTOR.file.pool
            is not peer.request.DESCRIPTOR.file.pool
        )
        try:
            descriptor_pool.Default().FindMessageTypeByName(name)
        except KeyError:
            pass
        else:
            raise AssertionError("producer leaked into the default protobuf pool")
        try:
            operation.request().MergeFrom(peer.request())
        except (TypeError, ValueError):
            pass
        else:
            raise AssertionError("cross-producer class identity was accepted")
    projector = _projector(first)
    left, right = projector(_contract()), projector(_contract())
    assert left.request is not right.request
    assert left.request.DESCRIPTOR.full_name == right.request.DESCRIPTOR.full_name
    assert left.request.DESCRIPTOR.file.pool is not right.request.DESCRIPTOR.file.pool


def typing(target: Path, alpha: Path, beta: Path) -> None:
    """Generated Stubs Accept Typed Consumers And Reject Mistyped Ones."""
    _typing(target, alpha / "src", beta / "src")


def install(target: Path, alpha: Path, beta: Path) -> None:
    """Wheel And Sdist Build Offline And Import Without The Producer Source."""
    metadata = tomllib.loads((alpha / "pyproject.toml").read_text())
    assert metadata["project"]["version"] == "1.2.3rc4"
    assert metadata["project"]["dependencies"] == ["protobuf>=7.36.2"]
    assert (alpha / "src/bindings_alpha/py.typed").is_file()
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
        for name in ("py.typed", "__init__.pyi", "contracts.json", "descriptors.bin"):
            assert f"bindings_alpha/{name}" in names
        assert any(name.endswith("_pb2.pyi") for name in names)
        assert not any(name.endswith((".so", ".pyd", ".dll")) for name in names)
    with tarfile.open(sdist) as archive:
        assert not any(
            name.endswith(("Cargo.toml", ".rs")) for name in archive.getnames()
        )
    # Remove Producer Source From Both Installations' Import And Build Paths
    held = target / "producer-source-held"
    alpha.rename(held)
    for archive, destination in (
        (wheel, "wheel-installed"),
        (sdist, "sdist-installed"),
    ):
        destination = target / destination
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
        _run(
            [sys.executable, "-c", INSTALLED, str(destination), str(beta / "src")],
            env=env,
            cwd=target,
        )
        _typing(target, destination, beta / "src")
    held.rename(alpha)


def dynamic(target: Path, alpha: Path, beta: Path) -> None:
    """A Contract Projects At Runtime Into Every Streaming Mode."""
    projector = _projector(_imported(alpha, beta)[0])
    _graph_projection(projector)
    _unary_projection(projector)
    _reserved_members(projector)


def refusals(target: Path, alpha: Path, beta: Path) -> None:
    """Malformed Graphs, Exceeded Budgets And Foreign Codecs Are Refused."""
    loader = _projector(_imported(alpha, beta)[0])
    for name, value in [("root", "0"), ("root", True), ("definitions", {})]:
        bad = _contract()
        bad["schema"][name] = value
        _raises(TypeError, loader, bad)
    bad = _contract()
    bad["schema"]["definitions"][1] = {}
    _raises(TypeError, loader, bad)
    bad = _contract()
    bad["schema"]["definitions"][1][2]["kind"]["definition"] = 99
    _raises(ValueError, loader, bad)
    bad = _contract()
    bad["schema"]["definitions"][0][0]["kind"] = {"type": "u32"}
    _raises(ValueError, loader, bad)
    bad = _contract()
    bad["schema"]["definitions"].append([])
    _raises(ValueError, loader, bad)
    bad = _contract()
    bad["schema"]["definitions"][1][1]["tag"] = 1
    _raises(ValueError, loader, bad)
    _raises(ValueError, lambda value: loader(value, max_work=1), _contract())
    _raises(ValueError, lambda value: loader(value, max_bytes=1), _contract())
    bad = _contract()
    bad["codec"] = "legacy"
    _raises(ValueError, loader, bad)


def aggregate(source: Path) -> None:
    """Read A Rust Summary, Check Its Typed Members, Then Rebuild It From Python."""
    sys.path.insert(0, str(source))
    bindings = importlib.import_module("bindings_aggregate")
    received = bindings.FixtureAggregateRequest.FromString(sys.stdin.buffer.read())
    assert received.aggregate.WhichOneof("_harmos_variant") == "Percentile"
    assert received.aggregate.Percentile.rank == 95
    variants = [item.WhichOneof("_harmos_variant") for item in received.aggregates]
    assert variants == ["Count", "Percentile"]
    built = bindings.FixtureAggregateRequest()
    built.aggregate.Percentile.rank = 95
    built.aggregates.add().Count.SetInParent()
    built.aggregates.add().Percentile.rank = 99
    sys.stdout.buffer.write(built.SerializeToString())


# === Private Functions === #

INSTALLED = """
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


def _imported(alpha: Path, beta: Path) -> tuple[types.ModuleType, types.ModuleType]:
    sys.path[:0] = [str(alpha / "src"), str(beta / "src")]
    return (
        importlib.import_module("bindings_alpha"),
        importlib.import_module("bindings_beta"),
    )


def _operations(package: types.ModuleType) -> list[Any]:
    return [
        value
        for value in vars(package).values()
        if isinstance(value, package.Operation)
    ]


def _projector(package: types.ModuleType) -> Callable[..., Any]:
    return importlib.import_module(package.__name__ + ".dynamic").from_contract


def _raises(error: type[Exception], call: Callable[[Any], Any], value: Any) -> None:
    try:
        call(value)
    except error:
        return
    raise AssertionError(f"{error.__name__} was not raised")


def _reserved(package: types.ModuleType, operation: Any) -> None:
    # The Generated And The Runtime Projection Escape A Reserved Member Alike
    contract = json.loads(operation.contract)
    raw = contract["schema"]["definitions"][0][0]
    assert raw["name"] == "Clear" and raw["tag"] == 7
    projected = _projector(package)(contract)
    for candidate in (operation, projected):
        field = candidate.request.DESCRIPTOR.fields_by_number[7]
        assert field.name == "Clear_" and field.json_name == "Clear"
        value = candidate.request()
        value.Clear_.SetInParent()
        assert value.SerializeToString() == b"\x3a\x00"
        assert json_format.MessageToDict(value) == {"Clear": {}}
        parsed = candidate.request()
        json_format.ParseDict({"Clear": {}}, parsed)
        assert parsed.SerializeToString() == value.SerializeToString()
        assert callable(parsed.Clear)
        parsed.Clear()
        assert parsed.SerializeToString() == b""


def _graph(pool: descriptor_pool.DescriptorPool, schema: dict[str, Any]) -> None:
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


def _typing(target: Path, alpha: Path, beta: Path) -> None:
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


def _graph_projection(projector: Callable[..., Any]) -> None:
    operation = projector(_contract())
    assert operation.grpc_path == "/external.v1.Plugin/Exchange"
    assert operation.mode == "bidi"
    value = operation.request()
    json_format.ParseDict(
        {"wide": "18446744073709551615", "optional": 0, "next": {"wide": "9"}},
        value,
    )
    assert value.HasField("optional")
    assert value.wide == 18446744073709551615
    assert value.next.wide == 9
    request = operation.rpc_request(request=value)
    assert request.WhichOneof("value") == "request"
    assert operation.rpc_request.FromString(request.SerializeToString()) == request
    assert (
        operation.rpc_request.DESCRIPTOR.full_name == "external.v1.PluginExchangeInput"
    )
    item = operation.output(node=value)
    output = operation.rpc_response(item=item)
    assert output.WhichOneof("value") == "item"
    assert item.WhichOneof("_harmos_variant") == "node"
    completion = operation.rpc_response(response=operation.response())
    assert completion.WhichOneof("value") == "response"


def _unary_projection(projector: Callable[..., Any]) -> None:
    contract = _contract()
    contract["schema"]["definitions"][0] = contract["schema"]["definitions"][0][:2]
    contract["schema"]["definitions"].pop()
    operation = projector(contract)
    assert operation.mode == "unary"
    assert operation.input is None and operation.output is None
    assert operation.rpc_request is operation.request
    assert operation.rpc_response is operation.response
    for direction, mode in (("input", "client_stream"), ("output", "server_stream")):
        contract = _contract()
        roots = contract["schema"]["definitions"][0]
        roots[:] = roots[:2] + [
            next(field for field in roots if field["name"] == direction)
        ]
        roots[2]["tag"] = 3
        if direction == "input":
            contract["schema"]["definitions"].pop()
        operation = projector(contract)
        assert operation.mode == mode
        service = operation.request.DESCRIPTOR.file.pool.FindServiceByName(
            "external.v1.Plugin"
        )
        method = service.methods_by_name["Exchange"]
        assert method.client_streaming == (direction == "input")
        assert method.server_streaming == (direction == "output")
        if direction == "input":
            request = operation.rpc_request(request=operation.request())
            assert request.WhichOneof("value") == "request"
            assert operation.rpc_response is operation.response
        else:
            assert operation.rpc_request is operation.request
            response = operation.rpc_response(response=operation.response())
            assert response.WhichOneof("value") == "response"


def _reserved_members(projector: Callable[..., Any]) -> None:
    contract = _contract()
    fields = contract["schema"]["definitions"][1]
    fields[0]["name"] = "Clear"
    operation = projector(contract)
    descriptor = operation.request.DESCRIPTOR.fields_by_number[fields[0]["tag"]]
    assert (descriptor.name, descriptor.json_name) == ("Clear_", "Clear")
    value = operation.request(Clear_=7)
    assert json_format.MessageToDict(value) == {"Clear": "7"}
    assert fields[0]["name"] == "Clear"
    collision = copy.deepcopy(contract)
    duplicate = copy.deepcopy(fields[0])
    duplicate.update(name="Clear_", tag=77)
    collision["schema"]["definitions"][1].append(duplicate)
    try:
        projector(collision)
    except ValueError as error:
        assert "collide after" in str(error)
    else:
        raise AssertionError("escaped member names were allowed to collide")
    module = importlib.import_module(projector.__module__)
    members = {name for name in dir(Message) if not name.startswith("_")}
    assert members <= module.RESERVED_FIELDS


def _contract() -> dict[str, Any]:
    def field(
        name: str, tag: int, kind: Any, cardinality: str = "required"
    ) -> dict[str, Any]:
        return {
            "name": name,
            "tag": tag,
            "kind": {"type": kind} if isinstance(kind, str) else kind,
            "cardinality": cardinality,
        }

    def message(index: int) -> dict[str, Any]:
        return {"type": "message", "definition": index}

    return copy.deepcopy(
        {
            "operation": {
                "id": "external.exchange",
                "description": "",
                "policy": {
                    "timeout": {"secs": 1, "nanos": 0},
                    "attempt_timeout": {"secs": 1, "nanos": 0},
                    "retries": "never",
                    "restart": "never",
                },
                "visibility": "shared",
                "replay": "idempotent",
            },
            "version": 1,
            "codec": "postcard",
            "grpc": {
                "package": "external.v1",
                "service": "Plugin",
                "method": "Exchange",
            },
            "schema": {
                "root": 0,
                "definitions": [
                    [
                        field("request", 1, message(1)),
                        field("response", 2, message(2)),
                        field("input", 3, message(1)),
                        field("output", 4, message(3)),
                    ],
                    [
                        field("wide", 1, "u64"),
                        field("optional", 2, "u32", "optional"),
                        field("next", 3, message(1), "optional"),
                    ],
                    [],
                    [
                        field("node", 1, message(1), "variant"),
                        field("empty", 2, message(2), "variant"),
                    ],
                ],
            },
        }
    )


# === Entry Point === #

if __name__ == "__main__":
    scene, *paths = sys.argv[1:]
    arguments = [Path(path) for path in paths]
    {
        "wire": wire,
        "descriptors": descriptors,
        "isolation": isolation,
        "typing": typing,
        "install": install,
        "dynamic": dynamic,
        "refusals": refusals,
        "aggregate": aggregate,
    }[scene](*arguments)
