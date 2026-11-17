# ***
#
# Harmos
#
# SPDX-License-Identifier: Apache-2.0
#
# ***

"""Render One Nested Contract Through Every Shared Language Backend."""

# === Imports === #

# Standard Library Imports
import ast
import asyncio
import tempfile
import unittest
from pathlib import Path
from types import SimpleNamespace
from typing import Any, cast
from unittest.mock import patch

# Third Party Imports
from google.protobuf.descriptor_pb2 import FieldDescriptorProto, FileDescriptorSet
from google.protobuf.descriptor_pool import DescriptorPool
from google.protobuf.message import Message
from google.protobuf.message_factory import GetMessageClass

# Local Imports
from harmos_tools.build.resources import cpp, csharp, descriptor, python, rust, sdk
from harmos_tools.build.resources.configuration import Configuration
from harmos_tools.source.protocol import caller_options, narrow_callers

# Type Checking Imports

# === Public Exports === #

# === Constants === #

ROOT = Path(__file__).resolve().parents[4]

# === Types === #

# === Classes === #


class SharedGeneratorPreservesContracts(unittest.TestCase):
    def test_nested_values_presence_audiences_and_call_options(self) -> None:
        # Assemble a Product Contract With the Real Caller Extension.
        library = FileDescriptorSet.FromString((ROOT / "crates/harmos-protocol/src/descriptors.bin").read_bytes())
        declarations = caller_options(library)
        options = {}
        identity = next(identity for identity, audience in declarations.items() if audience == "ALL")
        for audience in ("CLIENTS", "PLUGINS", "ALL"):
            selected = FileDescriptorSet.FromString(library.SerializeToString())
            narrow_callers(selected, {identity: audience})
            options[audience] = next(
                method.options
                for file in selected.file
                for service in file.service
                for method in service.method
                if f"{file.package}.{service.name}/{method.name}" == identity
            )
        schema = FileDescriptorSet.FromString(library.SerializeToString())
        file = schema.file.add(name="example/host/v1/nested/jobs.proto", package="example.host.v1.nested", syntax="proto3")
        file.dependency.append("harmos/model/v1/callers.proto")
        file.options.csharp_namespace = "Example.Clients"
        outer = file.message_type.add(name="Envelope")
        enum = outer.enum_type.add(name="Mode")
        enum.value.add(name="MODE_UNSPECIFIED", number=0)
        enum.value.add(name="MODE_ACTIVE", number=1)
        request = outer.nested_type.add(name="Input")
        request.oneof_decl.add(name="_count")
        request.field.add(name="count", number=1, type=FieldDescriptorProto.TYPE_INT32, proto3_optional=True, oneof_index=0)
        request.field.add(name="class", number=2, type=FieldDescriptorProto.TYPE_STRING)
        request.field.add(name="type", number=3, type=FieldDescriptorProto.TYPE_INT32)
        request.field.add(
            name="modes",
            number=4,
            type=FieldDescriptorProto.TYPE_ENUM,
            type_name=".example.host.v1.nested.Envelope.Mode",
            label=FieldDescriptorProto.LABEL_REPEATED,
        )
        response = outer.nested_type.add(name="Output")
        response.field.add(name="items", number=1, type=FieldDescriptorProto.TYPE_STRING, label=FieldDescriptorProto.LABEL_REPEATED)
        service = file.service.add(name="Jobs")
        for name, audience in [("Delete", "ALL"), ("List", "CLIENTS"), ("Watch", "PLUGINS")]:
            method = service.method.add(
                name=name, input_type=".example.host.v1.nested.Envelope.Input", output_type=".example.host.v1.nested.Envelope.Output"
            )
            method.options.CopyFrom(options[audience])

        # Preserve Presence, Nested Enum Names and Independent Audiences.
        methods = [method for method in descriptor.table(schema) if method["owner"] == "example"]
        self.assertEqual(
            [(method["method"], method["face"]) for method in methods],
            [("Delete", "client"), ("Delete", "context"), ("List", "client"), ("Watch", "context")],
        )
        count, _, _, modes = methods[0]["fields"]
        self.assertTrue(count["optional"])
        self.assertTrue(count["wire_optional"])
        self.assertTrue(modes["repeated"])
        self.assertEqual(modes["csharp"], "Example.Clients.Envelope.Types.Mode")
        self.assertIn("example.host.v1.nested.Envelope.Input", descriptor.payload_audiences(schema))
        self.assertEqual(descriptor.payload_audiences(schema)["example.host.v1.nested.Envelope"], {"client", "plugin"})
        narrowed = FileDescriptorSet.FromString(schema.SerializeToString())
        narrow_callers(narrowed, {"example.host.v1.nested.Jobs/Delete": "CLIENTS"})
        selected = [method for method in descriptor.table(narrowed) if method["owner"] == "example" and method["method"] == "Delete"]
        self.assertEqual([method["face"] for method in selected], ["client"])
        self.assertEqual(caller_options(schema)["example.host.v1.nested.Jobs/Delete"], "ALL")

        # Render All Four Languages Without Reading a Product Checkout.
        scratch = tempfile.TemporaryDirectory()
        self.addCleanup(scratch.cleanup)
        output = Path(scratch.name)
        configuration = Configuration(namespace="example", library=library)
        clients = [method for method in methods if method["face"] == "client"]
        python.client(output / "client.py", clients, schema, configuration=configuration)
        rust.client(output, clients, target=output / "client.rs", configuration=configuration)
        csharp.client(output, clients, namespace="example", target=output / "Client.cs", configuration=configuration)
        cpp.client(output, clients, namespace="example", target=output / "client.hpp", configuration=configuration)
        parsed = ast.parse((output / "client.py").read_text())
        self.assertFalse(any(isinstance(node, ast.alias) and node.asname for node in ast.walk(parsed)))
        signature = next(node for node in ast.walk(parsed) if isinstance(node, ast.AsyncFunctionDef) and node.name == "delete")
        self.assertIn("class_", [argument.arg for argument in signature.args.args])
        self.assertIn("call_metadata", [argument.arg for argument in signature.args.kwonlyargs])
        self.assertIn("r#type", (output / "client.rs").read_text())
        self.assertIn("example_protocol::host::nested::envelope::Input", (output / "client.rs").read_text())
        self.assertIn("into_iter().map(|value| value as i32).collect()", (output / "client.rs").read_text())
        self.assertIn("@class", (output / "Client.cs").read_text())
        self.assertIn(".Modes.Add(modes)", (output / "Client.cs").read_text())
        self.assertIn("set_class_", (output / "client.hpp").read_text())

        # Generate and Check the Complete SDK Inventory Without Requiring a C# Compiler.
        source = output / "product"
        source.mkdir()
        descriptor_path = source / "descriptors.bin"
        descriptor_path.write_bytes(schema.SerializeToString())
        composition = source / "composition.toml"
        composition.write_text('[[libraries]]\ndescriptors = "descriptors.bin"\npackages = ["harmos.model.v1"]\n', encoding="utf-8")
        full = Configuration(
            namespace="example",
            library=library,
            csharp_tools=source / "compiler/tools",
            rustfmt=ROOT / "sdk/rust/rustfmt.toml",
            python_style=ROOT / "sdk/python/pyproject.toml",
        )
        projection = patch("harmos_tools.build.resources.sdk.csharp_protocol.protocols")
        compiler = projection.start()
        self.addCleanup(projection.stop)
        sdk.generate(root=source, descriptors=descriptor_path, composition=composition, configuration=full)
        self.assertEqual(compiler.call_args.kwargs["descriptors"], descriptor_path)
        self.assertEqual(compiler.call_args.kwargs["tools"], full.csharp_tools)
        host_source = (source / "sdk/cpp/src/generated/host.cpp").read_text()
        self.assertIn("using ::example::host::v1::nested::Envelope;", host_source)
        self.assertIn("Operation<Envelope::Input, Envelope::Output>", host_source)
        self.assertNotIn("using ::example::host::v1::nested::Envelope::Input;", host_source)
        host_source = (source / "sdk/csharp/src/Example/Generated/Host.g.cs").read_text()
        self.assertIn("request.Modes.Add(modes)", host_source)
        self.assertNotIn("Modes = modes", host_source)
        self.assertIn("request.Count = count.Value", host_source)
        self.assertIn("context.CallAsync(HostContracts.NestedJobsDelete", host_source)
        inventory = {path: (source / path).read_bytes() for path in sdk.inventory(source, full)}
        sdk.generate(True, root=source, descriptors=descriptor_path, composition=composition, configuration=full)
        self.assertEqual(inventory, {path: (source / path).read_bytes() for path in sdk.inventory(source, full)})
        obsolete = source / "sdk/cpp/include/example/_generated/obsolete.hpp"
        obsolete.write_bytes(b"obsolete")
        self.assertRaises(
            ValueError, sdk.generate, True, root=source, descriptors=descriptor_path, composition=composition, configuration=full
        )
        self.assertTrue(obsolete.exists())
        sdk.generate(root=source, descriptors=descriptor_path, composition=composition, configuration=full)
        self.assertFalse(obsolete.exists())

        # Execute the Generated Python Call Against Real Protobuf Values.
        pool = DescriptorPool()
        pending = {file.name: file for file in schema.file}
        while pending:
            ready = [file for file in pending.values() if all(name not in pending for name in file.dependency)]
            for file in ready:
                pool.AddSerializedFile(file.SerializeToString())
                del pending[file.name]
        envelope = GetMessageClass(pool.FindMessageTypeByName("example.host.v1.nested.Envelope"))
        calls: list[tuple[Message, dict[str, object]]] = []

        async def unary(channel: object, path: str, value: Message, response: type[Message], **options: object) -> Message:
            calls.append((value, options))
            return response(items=["accepted"])

        class Connection:
            def __init__(self, channel: object) -> None:
                pass

        environment: dict[str, Any] = {
            "Envelope": envelope,
            "Channel": object,
            "CallCredentials": object,
            "Metadata": dict,
            "Sequence": list,
            "_connection": SimpleNamespace(Connection=Connection),
            "unary": unary,
        }
        parsed.body = [node for node in parsed.body if not isinstance(node, (ast.Import, ast.ImportFrom))]
        exec(compile(parsed, "generated-client", "exec"), environment)
        client = environment["ClientRoot"](object())
        result = asyncio.run(client.nested.jobs.delete("value", 7, [1], count=None, call_metadata={"token": "opaque"}, timeout=2.5))
        self.assertEqual(result, ["accepted"])
        self.assertFalse(calls[0][0].HasField("count"))
        self.assertEqual(getattr(calls[0][0], "class"), "value")
        self.assertEqual(list(cast(Any, calls[0][0]).modes), [1])
        self.assertEqual(calls[0][1]["metadata"], {"token": "opaque"})
        self.assertEqual(calls[0][1]["timeout"], 2.5)
        asyncio.run(client.nested.jobs.delete("value", 7, [1], count=0))
        self.assertTrue(calls[1][0].HasField("count"))
        self.assertEqual(cast(Any, calls[1][0]).count, 0)


# === Public Functions === #

# === Private Functions === #

# === Entry Point === #
