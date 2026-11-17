# ***
#
# Harmos
#
# SPDX-License-Identifier: Apache-2.0
#
# ***


"""Verify C++ Resource Parameters Avoid Member Names"""

# === Imports === #

# Standard Library Imports
import re
import tempfile
import unittest
from pathlib import Path

# Third Party Imports
from google.protobuf.descriptor_pb2 import FieldDescriptorProto, FileDescriptorSet

# Local Imports
from harmos_tools.build.resources import cpp, descriptor

# Type Checking Imports


# === Public Exports === #

# === Constants === #

# === Types === #

# === Classes === #


class CppResourcesAvoidShadowing(unittest.TestCase):
    def test_constructor_and_call_parameters_avoid_members_and_each_other(self) -> None:
        schema = FileDescriptorSet()
        file = schema.file.add(name="example/fixtures/channels.proto", package="example.host.v1", syntax="proto3")
        request = file.message_type.add(name="Input")
        fields = ["channel", "channel_", "context", "context_", "value", "options", "max_item_bytes", "request", "request_"]
        for number, name in enumerate(fields, 1):
            request.field.add(name=name, json_name=name, number=number, type=FieldDescriptorProto.TYPE_STRING)
        file.message_type.add(name="Output")
        service = file.service.add(name="Channels")
        service.method.add(name="Get", input_type=".example.host.v1.Input", output_type=".example.host.v1.Output")
        children = ["Channel_", "ChannelValue", "Context_", "ContextValue", "Value", "Options", "MaxItemBytes"]
        nested = schema.file.add(name="example/fixtures/children.proto", package="example.host.v1.channels", syntax="proto3")
        nested.dependency.append(file.name)
        for child in children:
            service = nested.service.add(name=child)
            service.method.add(name="Get", input_type=".example.host.v1.Input", output_type=".example.host.v1.Output")
        methods = descriptor.table(schema)
        scratch = tempfile.TemporaryDirectory()
        self.addCleanup(scratch.cleanup)
        root = Path(scratch.name)
        cpp.client(root, methods, target=root / "client.hpp")
        cpp.host(root, methods, target=root / "host.hpp")
        members = {"get", "channel_", "channel_value", "context_", "context_value", "value", "options", "max_item_bytes"}
        for filename, resource, transport in [
            ("client.hpp", "ChannelsResource", "channel"),
            ("host.hpp", "HostChannelsResource", "context"),
        ]:
            source = (root / filename).read_text()
            start = source.index(f"struct {resource} {{")
            body = source[start : source.index("\n};", start)]
            constructor = re.search(rf"{resource}\(([^)]*)\)", body)
            call = re.search(r"\bget\((.*?)\)\s*(?:;|\{)", body, re.DOTALL)
            assert constructor is not None and call is not None
            for signature in [constructor.group(1), call.group(1)]:
                parameters = re.findall(r"(\w+)\s*(?:=\s*\{\})?\s*(?:,|$)", signature)
                self.assertTrue(parameters)
                self.assertEqual(len(parameters), len(set(parameters)))
                self.assertFalse((members | {transport}).intersection(parameters), (filename, parameters))
                self.assertFalse(any("__" in parameter for parameter in parameters), (filename, parameters))


# === Public Functions === #

# === Private Functions === #

# === Entry Point === #
