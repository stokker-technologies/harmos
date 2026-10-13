# ***
#
# Harmos
#
# SPDX-License-Identifier: Apache-2.0
#
# ***

"""Live Finite Graphs Share Canonical Protobuf And Stream Descriptors"""

# === Imports === #

# Standard Library Imports
import copy
import importlib
import sys
import types
import unittest
from pathlib import Path
from collections.abc import Callable
from typing import Any

# Third Party Imports
from google.protobuf import json_format

# Local Imports

# Type Checking Imports

# === Public Exports === #

# === Constants === #
ROOT = Path(__file__).resolve().parents[4]

# === Types === #

# === Classes === #


class DynamicTests(unittest.TestCase):
    def test_dynamic_projection(self) -> None:
        for scenario in (
            self._graph_projection,
            self._pool_isolation,
            self._graph_refusals,
            self._unary_projection,
        ):
            with self.subTest(scenario=scenario.__name__):
                scenario()

    def _graph_projection(self) -> None:
        operation = _projector()(_contract())
        self.assertEqual(operation.grpc_path, "/external.v1.Plugin/Exchange")
        self.assertEqual(operation.mode, "bidi")
        value = operation.request()
        json_format.ParseDict(
            {"wide": "18446744073709551615", "optional": 0, "next": {"wide": "9"}},
            value,
        )
        self.assertTrue(value.HasField("optional"))
        self.assertEqual(value.wide, 18446744073709551615)
        self.assertEqual(value.next.wide, 9)
        request = operation.rpc_request(request=value)
        self.assertEqual(request.WhichOneof("value"), "request")
        self.assertEqual(
            operation.rpc_request.FromString(request.SerializeToString()), request
        )
        self.assertEqual(
            operation.rpc_request.DESCRIPTOR.full_name,
            "external.v1.PluginExchangeInput",
        )
        item = operation.output(node=value)
        output = operation.rpc_response(item=item)
        self.assertEqual(output.WhichOneof("value"), "item")
        self.assertEqual(item.WhichOneof("_harmos_variant"), "node")
        completion = operation.rpc_response(response=operation.response())
        self.assertEqual(completion.WhichOneof("value"), "response")

    def _pool_isolation(self) -> None:
        first, second = _projector()(_contract()), _projector()(_contract())
        self.assertIsNot(first.request, second.request)
        self.assertEqual(
            first.request.DESCRIPTOR.full_name, second.request.DESCRIPTOR.full_name
        )
        self.assertIsNot(
            first.request.DESCRIPTOR.file.pool, second.request.DESCRIPTOR.file.pool
        )

    def _graph_refusals(self) -> None:
        loader = _projector()
        bad = _contract()
        bad["schema"]["definitions"][1][2]["kind"]["definition"] = 99
        with self.assertRaises(ValueError):
            loader(bad)
        bad = _contract()
        bad["schema"]["definitions"][0][0]["kind"] = {"type": "u32"}
        with self.assertRaises(ValueError):
            loader(bad)
        bad = _contract()
        bad["schema"]["definitions"].append([])
        with self.assertRaises(ValueError):
            loader(bad)
        bad = _contract()
        bad["schema"]["definitions"][1][1]["tag"] = 1
        with self.assertRaises(ValueError):
            loader(bad)
        with self.assertRaises(ValueError):
            loader(_contract(), max_work=1)
        with self.assertRaises(ValueError):
            loader(_contract(), max_bytes=1)
        bad = _contract()
        bad["codec"] = "legacy"
        with self.assertRaises(ValueError):
            loader(bad)

    def _unary_projection(self) -> None:
        contract = _contract()
        contract["schema"]["definitions"][0] = contract["schema"]["definitions"][0][:2]
        contract["schema"]["definitions"].pop()
        operation = _projector()(contract)
        self.assertEqual(operation.mode, "unary")
        self.assertIsNone(operation.input)
        self.assertIsNone(operation.output)
        self.assertIs(operation.rpc_request, operation.request)
        self.assertIs(operation.rpc_response, operation.response)
        for direction, mode in (
            ("input", "client_stream"),
            ("output", "server_stream"),
        ):
            with self.subTest(direction=direction):
                contract = _contract()
                roots = contract["schema"]["definitions"][0]
                roots[:] = roots[:2] + [
                    next(field for field in roots if field["name"] == direction)
                ]
                roots[2]["tag"] = 3
                if direction == "input":
                    contract["schema"]["definitions"].pop()
                operation = _projector()(contract)
                self.assertEqual(operation.mode, mode)
                method = operation.request.DESCRIPTOR.file.pool.FindServiceByName(
                    "external.v1.Plugin"
                ).methods_by_name["Exchange"]
                self.assertEqual(method.client_streaming, direction == "input")
                self.assertEqual(method.server_streaming, direction == "output")
                if direction == "input":
                    self.assertEqual(
                        operation.rpc_request(request=operation.request()).WhichOneof(
                            "value"
                        ),
                        "request",
                    )
                    self.assertIs(operation.rpc_response, operation.response)
                else:
                    self.assertIs(operation.rpc_request, operation.request)
                    self.assertEqual(
                        operation.rpc_response(
                            response=operation.response()
                        ).WhichOneof("value"),
                        "response",
                    )


# === Public Functions === #

# === Private Functions === #


def _projector() -> Callable[..., Any]:
    name = "_harmos_dynamic_fixture"
    if name not in sys.modules:
        package = types.ModuleType(name)
        package.__dict__["__path__"] = [
            str(ROOT / "crates/harmos-contract/src/bindings/python")
        ]
        sys.modules[name] = package
    return importlib.import_module(name + ".dynamic").from_contract


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
